package com.countdowntodeath.app

import android.Manifest
import android.app.AlarmManager
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.SharedPreferences
import android.content.pm.PackageManager
import android.os.Build
import android.util.Log
import org.json.JSONArray
import org.json.JSONException
import org.json.JSONObject
import java.time.Instant
import java.time.ZoneId

internal object Reminders {
    const val CHANNEL_ID = "countdown_reminders"
    const val ALARM = "com.countdowntodeath.app.DELIVER_REMINDERS"
    private const val TAG = "CountdownReminders"
    private const val STATE = "state"
    private const val PLAN = "next_occurrences"
    private const val BACKUP_DELAY_MS = 60_000L
    private val lock = Any()

    private fun prefs(context: Context) = context.getSharedPreferences("countdown", Context.MODE_PRIVATE)
    private fun notifications(context: Context) = context.getSystemService(NotificationManager::class.java)
    private fun alarms(context: Context) = context.getSystemService(AlarmManager::class.java)

    fun ensureChannel(context: Context) {
        notifications(context).createNotificationChannel(NotificationChannel(
            CHANNEL_ID, "Countdown reminders", NotificationManager.IMPORTANCE_DEFAULT,
        ).apply { description = "Scheduled countdown updates and target-time notifications" })
    }

    fun notificationsEnabled(context: Context): Boolean {
        if (Build.VERSION.SDK_INT >= 33 && context.checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED) {
            return false
        }
        val manager = notifications(context)
        return manager.areNotificationsEnabled() && manager.getNotificationChannel(CHANNEL_ID)?.importance != NotificationManager.IMPORTANCE_NONE
    }

    fun exactAlarmsEnabled(context: Context): Boolean =
        Build.VERSION.SDK_INT < 31 || alarms(context).canScheduleExactAlarms()

    fun getState(context: Context): String = prefs(context).getString(STATE, "").orEmpty()

    fun saveState(context: Context, json: String) {
        if (json.length > 512_000) {
            Log.w(TAG, "Rejected oversized countdown state")
            return
        }
        val state = parseState(json) ?: return
        if (state.optJSONArray("profiles") == null) return
        synchronized(lock) { reconcile(context, state, json, false) }
    }

    fun refresh(context: Context, rebase: Boolean = false) = synchronized(lock) {
        reconcile(context, parseState(getState(context)) ?: JSONObject(), null, rebase)
    }

    private fun parseState(raw: String): JSONObject? = try {
        if (raw.isEmpty()) null else JSONObject(raw)
    } catch (_: JSONException) {
        null
    }

    private fun readPlan(prefs: SharedPreferences): List<Occurrence> = try {
        val array = JSONArray(prefs.getString(PLAN, "[]") ?: "[]")
        (0 until array.length()).map { i ->
            val item = array.getJSONArray(i)
            Occurrence(item.getString(0), Instant.ofEpochMilli(item.getLong(1)))
        }
    } catch (_: JSONException) {
        emptyList()
    }

    private fun reconcile(context: Context, state: JSONObject, rawState: String?, rebase: Boolean) {
        val prefs = prefs(context)
        val events = Schedule.events(state, ZoneId.systemDefault())
        val previous = if (rebase) emptyList() else readPlan(prefs)
        val plan = AlarmPlan.calculate(events, previous, Instant.now())
        val serialized = JSONArray().apply {
            plan.next.forEach { put(JSONArray().put(it.key).put(it.at.toEpochMilli())) }
        }.toString()
        val editor = prefs.edit().putString(PLAN, serialized)
        if (rawState != null) editor.putString(STATE, rawState)
        if (!editor.commit()) {
            Log.e(TAG, "Could not persist countdown state and alarm plan")
            return
        }
        arm(context, plan.next.minOfOrNull { it.at.toEpochMilli() })
        if (plan.due.isNotEmpty()) {
            val byKey = events.associateBy { it.key }
            ensureChannel(context)
            if (notificationsEnabled(context)) plan.due.forEach { deliver(context, byKey.getValue(it.key), it.at) }
        }
    }

    private fun alarmIntent(context: Context, requestCode: Int): PendingIntent = PendingIntent.getBroadcast(
        context, requestCode, Intent(context, ReminderReceiver::class.java).setAction(ALARM), PendingIntent.FLAG_IMMUTABLE,
    )

    private fun arm(context: Context, at: Long?) {
        val manager = alarms(context)
        val exact = alarmIntent(context, 0)
        val inexact = alarmIntent(context, 1)
        if (at == null) {
            manager.cancel(exact)
            manager.cancel(inexact)
            return
        }
        if (exactAlarmsEnabled(context)) {
            // Revoking exact access cancels exact alarms. Keep an inexact backup, replaced as soon as the exact one fires.
            manager.setAndAllowWhileIdle(AlarmManager.RTC_WAKEUP, at + BACKUP_DELAY_MS, inexact)
            try {
                manager.setExactAndAllowWhileIdle(AlarmManager.RTC_WAKEUP, at, exact)
                return
            } catch (_: SecurityException) {
                // Exact access can be revoked between the check and set.
            }
        }
        manager.cancel(exact)
        manager.setAndAllowWhileIdle(AlarmManager.RTC_WAKEUP, at, inexact)
    }

    private fun deliver(context: Context, event: Schedule.Event, at: Instant) {
        val locale = context.resources.configuration.locales[0]
        val message = if (event.reminder == null) CountdownText.COMPLETE else CountdownText.remaining(
            event.target, event.reminder.optJSONObject("format"), at, event.zone, locale,
        )
        val openApp = PendingIntent.getActivity(context, 0,
            Intent(context, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_CLEAR_TOP or Intent.FLAG_ACTIVITY_SINGLE_TOP),
            PendingIntent.FLAG_IMMUTABLE,
        )
        val notification = Notification.Builder(context, CHANNEL_ID)
            .setSmallIcon(R.drawable.ic_countdown)
            .setContentTitle(event.title)
            .setContentText(message)
            .setStyle(Notification.BigTextStyle().bigText(message))
            .setCategory(Notification.CATEGORY_REMINDER)
            .setVisibility(Notification.VISIBILITY_PRIVATE)
            .setWhen(at.toEpochMilli())
            .setContentIntent(openApp)
            .setAutoCancel(true)
            .build()
        try {
            notifications(context).notify(event.key, 0, notification)
        } catch (_: SecurityException) {
            Log.w(TAG, "Notification permission changed before delivery")
        }
    }
}

class ReminderReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        when (intent.action) {
            Reminders.ALARM, Intent.ACTION_MY_PACKAGE_REPLACED,
            AlarmManager.ACTION_SCHEDULE_EXACT_ALARM_PERMISSION_STATE_CHANGED -> Reminders.refresh(context)
            Intent.ACTION_BOOT_COMPLETED, Intent.ACTION_TIME_CHANGED, Intent.ACTION_TIMEZONE_CHANGED ->
                Reminders.refresh(context, rebase = true)
        }
    }
}
