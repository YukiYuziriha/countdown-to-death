package com.countdowntodeath.app

import android.app.Notification
import android.app.PendingIntent
import android.content.Context
import android.util.TypedValue
import android.widget.RemoteViews
import java.time.Instant

internal object ReminderNotification {
    fun build(
        context: Context,
        channelId: String,
        title: String,
        message: String,
        at: Instant,
        theme: String?,
        complete: Boolean,
        openApp: PendingIntent,
    ): Notification {
        val spectral = theme.equals("spectral", ignoreCase = true)
        val accent = context.getColor(
            if (spectral) R.color.reminder_spectral_accent else R.color.reminder_neon_accent,
        )
        val icon = R.drawable.ic_reminder_countdown
        val publicVersion = Notification.Builder(context, channelId)
            .setSmallIcon(icon)
            .setContentTitle(context.getString(R.string.reminder_notification_private))
            .setShowWhen(false)
            .build()

        val builder = Notification.Builder(context, channelId)
            .setSmallIcon(icon)
            .setContentTitle(title)
            .setContentText(message)
            .setColor(accent)
            .setCategory(Notification.CATEGORY_REMINDER)
            .setVisibility(Notification.VISIBILITY_PRIVATE)
            .setPublicVersion(publicVersion)
            .setWhen(at.toEpochMilli())
            .setShowWhen(true)
            .setContentIntent(openApp)
            .setAutoCancel(true)

        // Leave more room for text when the custom panel's padding and label would compete with it.
        val config = context.resources.configuration
        if (config.fontScale >= 1.3f || message.length > 180 || message.lineSequence().take(6).count() > 5) {
            builder.setStyle(Notification.BigTextStyle().bigText(message))
        } else {
            val layout = if (spectral) R.layout.reminder_notification_spectral else R.layout.reminder_notification_neon
            val messageSize = when {
                message.length <= 44 -> 20f
                message.length <= 100 && config.screenWidthDp >= 360 -> 17f
                else -> 15f
            }
            val expanded = RemoteViews(context.packageName, layout).apply {
                val label = if (complete) R.string.reminder_notification_complete else R.string.reminder_notification_status
                setTextColor(R.id.reminder_notification_status, accent)
                setTextViewText(R.id.reminder_notification_status, context.getText(label))
                setTextViewText(R.id.reminder_notification_title, title)
                setTextViewTextSize(R.id.reminder_notification_message, TypedValue.COMPLEX_UNIT_SP, messageSize)
                setTextViewText(R.id.reminder_notification_message, message)
            }
            builder.setCustomBigContentView(expanded)
                .setStyle(Notification.DecoratedCustomViewStyle())
        }
        return builder.build()
    }
}
