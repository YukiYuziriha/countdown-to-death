package com.countdowntodeath.app

import org.json.JSONObject
import java.text.NumberFormat
import java.time.Instant
import java.time.ZoneId
import java.util.Locale

internal object CountdownText {
    const val COMPLETE = "Time’s up. Countdown complete."
    private val fixedMillis = mapOf(
        "weeks" to 604_800_000L, "days" to 86_400_000L, "hours" to 3_600_000L,
        "minutes" to 60_000L, "seconds" to 1000L,
    )
    private val validUnits = setOf("years", "months") + fixedMillis.keys

    fun remaining(target: Instant, format: JSONObject?, intended: Instant, zone: ZoneId, locale: Locale): String {
        val configured = format?.optJSONArray("units")
        val units = (0 until (configured?.length() ?: 0)).mapNotNull { configured?.opt(it) as? String }
            .filter { it in validUnits }.distinct().ifEmpty { listOf("days", "hours", "minutes") }
        val numbers = NumberFormat.getNumberInstance(locale).apply { maximumFractionDigits = 1 }
        val end = target.toEpochMilli()
        val localEnd = target.atZone(zone)
        var cursor = minOf(intended.toEpochMilli(), end)
        return units.mapIndexed { index, unit ->
            val local = Instant.ofEpochMilli(cursor).atZone(zone)
            fun advance(count: Long): Long {
                if (count == 0L) return cursor
                val wallTime = local.toLocalDateTime()
                // Clamp on the local calendar and resolve DST only on the final date.
                return when (unit) {
                    "years" -> wallTime.plusYears(count).atZone(zone).toInstant().toEpochMilli()
                    "months" -> wallTime.plusMonths(count).atZone(zone).toInstant().toEpochMilli()
                    else -> cursor + count * fixedMillis.getValue(unit)
                }
            }
            var count = when (unit) {
                "years" -> (localEnd.year - local.year).toLong()
                "months" -> (localEnd.year - local.year) * 12L + localEnd.monthValue - local.monthValue
                else -> (end - cursor) / fixedMillis.getValue(unit)
            }.coerceAtLeast(0)
            while (count > 0 && advance(count) > end) count--
            val next = advance(count)
            // Intl.NumberFormat uses half-expand by default. Use decimal arithmetic for exact ties.
            val text = if (format?.opt("decimal") == true && index == units.lastIndex && end > next) {
                val span = advance(count + 1) - next
                val fraction = if (span > 0) (end - next).toBigDecimal()
                    .divide(span.toBigDecimal(), 1, java.math.RoundingMode.HALF_UP) else java.math.BigDecimal.ZERO
                numbers.format(count.toBigDecimal() + fraction)
            } else numbers.format(count)
            cursor = next
            "$text ${if (text == numbers.format(1)) unit.dropLast(1) else unit}"
        }.joinToString(", ", postfix = " left")
    }
}
