package com.countdowntodeath.app

import org.json.JSONArray
import org.json.JSONObject
import java.time.DateTimeException
import java.time.Instant
import java.time.LocalDate
import java.time.LocalDateTime
import java.time.LocalTime
import java.time.ZoneId

internal object Schedule {
    private val dateTimePattern = Regex("[0-9]{4,6}-[0-9]{2}-[0-9]{2}[T ]\\S+")
    private val timePattern = Regex("(?:[01][0-9]|2[0-3]):[0-5][0-9](?::[0-5][0-9](?:\\.[0-9]{1,3})?)?")
    private val weekdayPattern = Regex("[0-6]")
    private val monthDayPattern = Regex("[1-9]|[12][0-9]|3[01]")

    data class Event(
        val key: String,
        val title: String,
        val target: Instant,
        val reminder: JSONObject?,
        val zone: ZoneId,
    ) {
        fun next(after: Instant): Instant? {
            if (target <= after) return null
            if (reminder == null) return target
            val at = nextReminder(reminder, after, zone)
            return at?.takeIf { it > after && it < target }
        }
    }

    fun events(state: JSONObject, zone: ZoneId, includePaused: Boolean = false): List<Event> = buildList {
        val profiles = state.optJSONArray("profiles") ?: return@buildList
        for (i in 0 until profiles.length()) {
            val profile = profiles.optJSONObject(i) ?: continue
            val profileId = profile.string("id") ?: continue
            val target = localInstant(profile.string("target"), zone) ?: continue
            val title = profile.string("name").orEmpty().ifBlank { "Countdown to Death" }
            if (profile.opt("notifyAtTarget") == true) {
                add(Event(JSONArray().put(profileId).toString(), title, target, null, zone))
            }
            val reminders = profile.optJSONArray("reminders") ?: continue
            for (j in 0 until reminders.length()) {
                val reminder = reminders.optJSONObject(j) ?: continue
                val id = reminder.string("id") ?: continue
                if (includePaused || reminder.opt("enabled") == true) {
                    add(Event(JSONArray().put(profileId).put(id).toString(), title, target, reminder, zone))
                }
            }
        }
    }

    private fun nextReminder(reminder: JSONObject, after: Instant, zone: ZoneId): Instant? {
        val repeat = reminder.string("repeat")
        if (repeat == "once") return localInstant(reminder.string("once"), zone)
        val time = localTime(reminder.string("time")) ?: return null
        var date = after.atZone(zone).toLocalDate()
        var step = 1L
        var monthDay = 1
        when (repeat) {
            "daily" -> Unit
            "weekly" -> {
                val weekday = reminder.string("weekday")?.takeIf(weekdayPattern::matches)?.toInt() ?: return null
                date = date.plusDays(((weekday - date.dayOfWeek.value + 14) % 7).toLong())
                step = 7L
            }
            "monthly" -> {
                monthDay = reminder.string("day")?.takeIf(monthDayPattern::matches)?.toInt() ?: return null
                date = date.withDayOfMonth(1)
            }
            else -> return null
        }
        while (true) {
            val day = if (repeat == "monthly") date.withDayOfMonth(minOf(monthDay, date.lengthOfMonth())) else date
            // atZone shifts a gap forward and selects the earlier offset in an overlap, like JS Date.
            val instant = day.atTime(time).atZone(zone).toInstant()
            if (instant > after) return instant
            date = if (repeat == "monthly") date.plusMonths(1) else date.plusDays(step)
        }
    }

    private fun localTime(value: String?): LocalTime? = value?.takeIf(timePattern::matches)?.let(LocalTime::parse)

    private fun localInstant(value: String?, zone: ZoneId): Instant? {
        if (value == null || !dateTimePattern.matches(value)) return null
        val split = value.indexOfFirst { it == 'T' || it == ' ' }
        val time = localTime(value.substring(split + 1)) ?: return null
        val year = value.substring(0, split - 6).toInt()
        if (year !in 1..275760) return null
        val month = value.substring(split - 5, split - 3).toInt()
        val day = value.substring(split - 2, split).toInt()
        return try {
            val date = LocalDate.of(year, month, day)
            LocalDateTime.of(date, time).atZone(zone).toInstant()
                .takeIf { it.toEpochMilli() <= 8_640_000_000_000_000L }
        } catch (_: DateTimeException) {
            null
        }
    }
}

internal fun JSONObject.string(key: String): String? = opt(key) as? String
