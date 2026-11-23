package com.countdowntodeath.app

import java.time.Duration
import java.time.Instant

internal data class Occurrence(val key: String, val at: Instant)
internal data class AlarmPlan(val due: List<Occurrence>, val next: List<Occurrence>) {
    companion object {
        private val maximumLateness = Duration.ofHours(6)
        val earliest = compareBy<Occurrence> { it.at }.thenBy { it.key }

        fun calculate(
            events: List<Schedule.Event>, previous: List<Occurrence>, now: Instant,
            delivered: Map<String, Instant> = emptyMap(),
        ): AlarmPlan {
            val byKey = events.associateBy { it.key }
            val cutoff = now.minus(maximumLateness)
            val due = previous.filter { (key, at) ->
                val event = byKey[key]
                at in cutoff..now && (delivered[key]?.let { at > it } ?: true) &&
                    event != null && (event.reminder == null || now < event.target) &&
                    event.next(at.minusMillis(1L)) == at
            }.sortedWith(earliest).distinctBy { it.key }
            val next = byKey.values.mapNotNull { event ->
                val after = maxOf(now, delivered[event.key] ?: now)
                event.next(after)?.let { Occurrence(event.key, it) }
            }.sortedWith(earliest)
            return AlarmPlan(due, next)
        }
    }
}

internal inline fun armOneShot(
    exactAllowed: Boolean,
    shadow: () -> Unit,
    exact: () -> Unit,
    inexact: () -> Unit,
) {
    if (exactAllowed) {
        shadow()
        try {
            exact()
            return
        } catch (_: SecurityException) {
            // Access may have been revoked between the check and the exact call.
        }
    }
    inexact()
}
