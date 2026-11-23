package com.countdowntodeath.app

import android.Manifest
import android.app.AlarmManager
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.SharedPreferences
import android.content.pm.PackageManager
import android.net.Uri
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
    private const val DELIVERED = "delivered_occurrences"
    private const val IDENTITIES = "alarm_identity_version"
    private const val SHADOW_DELAY_MS = 60_000L
    private val lock = Any()

    private fun prefs(context: Context) = context.getSharedPreferences("countdown", Context.MODE_PRIVATE)
    private fun notifications(context: Context) = context.getSystemService(NotificationManager::class.java)
    private fun alarms(context: Context) = context.getSystemService(AlarmManager::class.java)

    fun ensureChannel(context: Context) {
        val manager = notifications(context)
        if (manager.getNotificationChannel(CHANNEL_ID) != null) return
        manager.createNotificationChannel(NotificationChannel(
            CHANNEL_ID, "Countdown reminders", NotificationManager.IMPORTANCE_DEFAULT,
        ).apply { description = "Scheduled countdown updates and target-time notifications" })
    }

    fun notificationBlock(context: Context): String? {
        if (Build.VERSION.SDK_INT >= 33 && context.checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED) {
            return "permission"
        }
        val manager = notifications(context)
        if (!manager.areNotificationsEnabled()) return "app"
        val channel = manager.getNotificationChannel(CHANNEL_ID)
        if (channel?.importance == NotificationManager.IMPORTANCE_NONE ||
            (Build.VERSION.SDK_INT >= 28 && channel?.group?.let { manager.getNotificationChannelGroup(it)?.isBlocked } == true)) return "channel"
        return null
    }

    fun exactAlarmsEnabled(context: Context): Boolean =
        Build.VERSION.SDK_INT < 31 || alarms(context).canScheduleExactAlarms()

    fun getState(context: Context): String = prefs(context).getString(STATE, "").orEmpty()

    fun saveState(context: Context, json: String) {
        require(json.length <= 512_000) { "Countdown state is too large" }
        val state = requireNotNull(parseState(json)) { "Invalid countdown state" }
        require(state.optJSONArray("profiles") != null) { "Missing countdown profiles" }
        synchronized(lock) { reconcile(context, state, json, rebase = false) }
    }

    fun refresh(context: Context, rebase: Boolean = false) = synchronized(lock) {
        reconcile(context, parseState(getState(context)) ?: JSONObject(), null, rebase)
    }

    fun alarmReceived(context: Context, data: Uri?) = synchronized(lock) {
        val stored = prefs(context)
        val pending = readOccurrences(stored, PLAN).minWithOrNull(AlarmPlan.earliest) ?: return@synchronized
        if (data != alarmUri(pending.key, "exact") && data != alarmUri(pending.key, "flex")) return@synchronized
        reconcile(context, parseState(getState(context)) ?: JSONObject(), null, rebase = false)
    }

    private fun parseState(raw: String): JSONObject? = try {
        if (raw.isEmpty()) null else JSONObject(raw)
    } catch (_: JSONException) {
        null
    }

    private fun readOccurrences(prefs: SharedPreferences, name: String): List<Occurrence> = try {
        val array = JSONArray(prefs.getString(name, "[]") ?: "[]")
        (0 until array.length()).map { i ->
            val item = array.getJSONArray(i)
            Occurrence(item.getString(0), Instant.ofEpochMilli(item.getLong(1)))
        }
    } catch (_: JSONException) {
        emptyList()
    }

    private fun serialize(items: Iterable<Occurrence>): String = JSONArray().apply {
        items.forEach { put(JSONArray().put(it.key).put(it.at.toEpochMilli())) }
    }.toString()

    private fun reconcile(context: Context, state: JSONObject, raw: String?, rebase: Boolean) {
        val stored = prefs(context)
        val previous = readOccurrences(stored, PLAN)
        val previousLeader = previous.minWithOrNull(AlarmPlan.earliest)
        val definitions = Schedule.events(state, ZoneId.systemDefault(), includePaused = true).associateBy { it.key }
        val active = definitions.values.filter { it.reminder == null || it.reminder.opt("enabled") == true }
        val delivered = readOccurrences(stored, DELIVERED).filter { it.key in definitions }
            .associate { it.key to it.at }.toMutableMap()
        val plan = if (notificationBlock(context) != null) AlarmPlan(emptyList(), emptyList()) else
            AlarmPlan.calculate(active, if (rebase) emptyList() else previous, Instant.now(), delivered)
        plan.due.forEach { delivered[it.key] = it.at }
        val next = serialize(plan.next)
        val history = serialize(delivered.map { (key, at) -> Occurrence(key, at) }.sortedBy { it.key })
        val migrate = stored.getInt(IDENTITIES, 0) == 0
        if (raw != null || migrate || next != stored.getString(PLAN, "[]") || history != stored.getString(DELIVERED, "[]")) {
            val editor = stored.edit().putString(PLAN, next).putString(DELIVERED, history).putInt(IDENTITIES, 1)
            if (raw != null) editor.putString(STATE, raw)
            // Record consumption before posting so repeated broadcasts cannot alert again.
            check(editor.commit()) { "Could not persist countdown state and alarm plan" }
        }
        if (migrate) {
            // v1.0.1 used two app-wide PendingIntents without a data URI.
            for (code in 0..1) {
                PendingIntent.getBroadcast(context, code, receiverIntent(context), lookupFlags)?.let { cancel(context, it) }
            }
        }
        val leader = plan.next.firstOrNull()
        if (leader != null && (previousLeader != leader || raw == null || migrate)) arm(context, leader)
        if (previousLeader != null && previousLeader.key != leader?.key) {
            for (kind in listOf("exact", "flex")) token(context, previousLeader.key, kind, lookupFlags)?.let { cancel(context, it) }
        }
        if (plan.due.isNotEmpty()) {
            ensureChannel(context)
            plan.due.forEach { deliver(context, definitions.getValue(it.key), it.at, state.string("theme")) }
        }
    }

    private val lookupFlags = PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_NO_CREATE
    private fun receiverIntent(context: Context) = Intent(context, ReminderReceiver::class.java).setAction(ALARM)
    private fun alarmUri(key: String, kind: String): Uri = Uri.Builder().scheme("countdown").authority("reminders")
        .appendPath(kind).appendPath(key).build()

    private fun token(context: Context, key: String, kind: String, flags: Int): PendingIntent? =
        PendingIntent.getBroadcast(context, 0, receiverIntent(context).setData(alarmUri(key, kind)), flags)

    private fun cancel(context: Context, intent: PendingIntent) {
        alarms(context).cancel(intent)
        intent.cancel()
    }

    private fun arm(context: Context, leader: Occurrence) {
        val manager = alarms(context)
        val at = leader.at.toEpochMilli()
        val flex = requireNotNull(token(context, leader.key, "flex", PendingIntent.FLAG_IMMUTABLE))
        // Only the earliest occurrence owns an alarm. A single delivery can post all due reminders.
        armOneShot(exactAlarmsEnabled(context),
            shadow = { manager.set(AlarmManager.RTC, at + SHADOW_DELAY_MS, flex) },
            exact = {
                val exact = requireNotNull(token(context, leader.key, "exact", PendingIntent.FLAG_IMMUTABLE))
                manager.setExactAndAllowWhileIdle(AlarmManager.RTC_WAKEUP, at, exact)
            },
            inexact = {
                token(context, leader.key, "exact", lookupFlags)?.let { cancel(context, it) }
                manager.set(AlarmManager.RTC_WAKEUP, at, flex)
            },
        )
    }

    private fun deliver(context: Context, event: Schedule.Event, at: Instant, theme: String?) {
        val locale = context.resources.configuration.locales[0]
        val message = if (event.reminder == null) CountdownText.COMPLETE else CountdownText.remaining(
            event.target, event.reminder.optJSONObject("format"), at, event.zone, locale,
        )
        val openApp = PendingIntent.getActivity(context, 0,
            Intent(context, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_CLEAR_TOP or Intent.FLAG_ACTIVITY_SINGLE_TOP),
            PendingIntent.FLAG_IMMUTABLE,
        )
        val notification = ReminderNotification.build(context, CHANNEL_ID, event.title, message, at, theme, event.reminder == null, openApp)
        try {
            notifications(context).notify(event.key, 0, notification)
        } catch (_: SecurityException) {
            Log.w(TAG, "Notification permission changed before delivery")
        }
    }
}

class ReminderReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        val action = intent.action ?: return
        val rebase = when (action) {
            Intent.ACTION_BOOT_COMPLETED, Intent.ACTION_MY_PACKAGE_REPLACED,
            Intent.ACTION_TIME_CHANGED, Intent.ACTION_TIMEZONE_CHANGED -> true
            Reminders.ALARM, AlarmManager.ACTION_SCHEDULE_EXACT_ALARM_PERMISSION_STATE_CHANGED -> false
            else -> return
        }
        val result = goAsync()
        val application = context.applicationContext
        val data = intent.data
        Thread {
            try {
                if (action == Reminders.ALARM) Reminders.alarmReceived(application, data)
                else Reminders.refresh(application, rebase)
            } catch (error: Exception) {
                Log.e("CountdownReminders", "Could not handle reminder broadcast", error)
            } finally {
                result.finish()
            }
        }.start()
    }
}
