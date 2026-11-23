package com.countdowntodeath.app

import java.time.Instant

internal data class Occurrence(val key: String, val at: Instant)
internal data class AlarmPlan(val due: List<Occurrence>, val next: List<Occurrence>) {
    companion object {
        fun calculate(events: List<Schedule.Event>, previous: List<Occurrence>, now: Instant): AlarmPlan {
            val byKey = events.associateBy { it.key }
            val due = previous.filter { (key, at) ->
                // Revalidate after state edits; a deleted/paused/changed occurrence must not fire.
                at <= now && byKey[key]?.next(at.minusMillis(1L)) == at
            }.sortedBy { it.at }
            val next = byKey.values.mapNotNull { event -> event.next(now)?.let { Occurrence(event.key, it) } }
            return AlarmPlan(due, next)
        }
    }
}
