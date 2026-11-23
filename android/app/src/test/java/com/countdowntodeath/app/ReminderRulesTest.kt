package com.countdowntodeath.app

import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.time.Instant
import java.time.ZoneId
import java.util.Locale

class ReminderRulesTest {
    private val utc = ZoneId.of("UTC")
    private val newYork = ZoneId.of("America/New_York")
    private fun instant(text: String) = Instant.parse(text)

    private fun events(
        rules: String,
        target: String = "2028-01-01T09:00",
        zone: ZoneId = utc,
    ) = Schedule.events(JSONObject("""{"profiles":[{"id":"p","name":"My countdown","target":"$target",
        "notifyAtTarget":true,"reminders":[$rules]}]}"""), zone)

    @Test fun monthlyClampsAndRestoresOriginalDay() {
        val reminder = events("""{"id":"last","enabled":true,"repeat":"monthly","time":"09:00","day":"31"}""").last()
        assertEquals(instant("2027-02-28T09:00:00Z"), reminder.next(instant("2027-02-01T00:00:00Z")))
        assertEquals(instant("2027-03-31T09:00:00Z"), reminder.next(instant("2027-02-28T09:00:00Z")))
        val leap = reminder.copy(target = instant("2029-01-01T09:00:00Z"))
        assertEquals(instant("2028-02-29T09:00:00Z"), leap.next(instant("2028-02-01T00:00:00Z")))
    }

    @Test fun dailyAndWeeklyResolveInCurrentZone() {
        val rules = """{"id":"daily","enabled":true,"repeat":"daily","time":"02:30"},
            {"id":"weekly","enabled":true,"repeat":"weekly","time":"01:30","weekday":"0"}"""
        val (_, daily, weekly) = events(rules, zone = newYork)
        assertEquals(instant("2027-03-14T07:30:00Z"), daily.next(instant("2027-03-14T06:00:00Z")))
        assertEquals(instant("2027-03-14T02:30:00Z"), events(rules)[1].next(instant("2027-03-14T01:00:00Z")))
        assertEquals(instant("2027-11-07T05:30:00Z"), weekly.next(instant("2027-11-06T12:00:00Z")))
        assertEquals(instant("2027-11-14T06:30:00Z"), weekly.next(instant("2027-11-07T05:30:00Z")))
        assertEquals(instant("2027-11-14T06:30:00Z"), weekly.next(instant("2027-11-08T12:00:00Z")))
    }

    @Test fun onceIsStrictlyFutureAndBeforeSeparateTarget() {
        val rules = """{"id":"once","enabled":true,"repeat":"once","once":"2027-12-31T09:00:00.123"},
            {"id":"equal","enabled":true,"repeat":"once","once":"2028-01-01T09:00"},
            {"id":"paused","enabled":false,"repeat":"daily","time":"09:00"},
            {"id":"invalid","enabled":true,"repeat":"once","once":"2027-02-29T09:00"}"""
        val all = events(rules)
        assertEquals(4, all.size)
        val now = instant("2027-01-01T00:00:00Z")
        val (target, once, equal, invalid) = all
        assertEquals(instant("2028-01-01T09:00:00Z"), target.next(now))
        val due = instant("2027-12-31T09:00:00.123Z")
        assertEquals(due, once.next(now))
        assertNull(once.next(due))
        assertNull(equal.next(now))
        assertNull(invalid.next(now))
        assertNull(target.next(target.target))
        assertTrue(Schedule.events(JSONObject("""{"profiles":[]}"""), utc).isEmpty())
    }

    @Test fun simultaneousRemindersAreConsumedOnceAndEditsInvalidateTheOldPlan() {
        val rules = """{"id":"first","enabled":true,"repeat":"daily","time":"09:00"},
            {"id":"second","enabled":true,"repeat":"daily","time":"09:00"}"""
        val all = events(rules, "2027-09-03T20:00")
        val before = AlarmPlan.calculate(all, emptyList(), instant("2027-09-02T08:00:00Z"))
        val at = instant("2027-09-02T09:00:00Z")
        val delivery = AlarmPlan.calculate(all, before.next, at.plusSeconds(300))
        assertEquals(listOf(at, at), delivery.due.map { it.at })
        val delivered = delivery.due.associate { it.key to it.at }
        assertTrue(AlarmPlan.calculate(all, before.next, at, delivered).due.isEmpty())
        assertTrue(AlarmPlan.calculate(all, emptyList(), at.minusSeconds(60), delivered).next.none { it.at == at })
        val changed = events(rules.replaceFirst("09:00", "10:00").replace("\"second\",\"enabled\":true", "\"second\",\"enabled\":false"), "2027-09-03T20:00")
        assertTrue(AlarmPlan.calculate(changed, before.next, at, delivered = emptyMap()).due.isEmpty())
    }

    @Test fun rebaseSkipsPastOccurrencesAndLateAlarmsNeverReplayTheBacklog() {
        val all = events("""{"id":"daily","enabled":true,"repeat":"daily","time":"09:00"}""", "2027-09-04T20:00")
        val before = AlarmPlan.calculate(all, emptyList(), instant("2027-09-02T08:00:00Z"))
        val now = instant("2027-09-04T19:30:00Z")
        val rebase = AlarmPlan.calculate(all, emptyList(), now)
        assertEquals(rebase, AlarmPlan.calculate(all, before.next, now))
        assertEquals(instant("2027-09-04T20:00:00Z"), rebase.next.single().at)
        val completion = AlarmPlan.calculate(all, rebase.next, instant("2027-09-04T20:01:00Z"))
        assertEquals(rebase.next, completion.due)
        assertTrue(completion.next.isEmpty())
    }

    @Test fun alarmAccessFallsBackIfDeniedOrRevokedDuringArm() {
        fun calls(allowed: Boolean, revoked: Boolean): List<String> = buildList {
            armOneShot(allowed, shadow = { add("shadow") }, exact = {
                add("exact")
                if (revoked) throw SecurityException()
            }, inexact = { add("inexact") })
        }
        assertEquals(listOf("shadow", "exact"), calls(true, false))
        assertEquals(listOf("inexact"), calls(false, false))
        assertEquals(listOf("shadow", "exact", "inexact"), calls(true, true))
    }

    @Test fun notificationFormattingUsesIntendedInstantAndOneDecimalOnLastUnit() {
        val at = instant("2027-01-01T09:00:00Z")
        val target = instant("2027-01-02T10:03:00Z")
        val fmt = JSONObject("""{"units":["days","hours"],"decimal":true}""")
        assertEquals("1 day, 1.1 hours left", CountdownText.remaining(target, fmt, at, utc, Locale.US))
        assertEquals("1 day, 1,1 hours left", CountdownText.remaining(target, fmt, at, utc, Locale.GERMANY))
        assertEquals("0 days, 0 hours left", CountdownText.remaining(target, fmt, target.plusSeconds(9), utc, Locale.US))
    }

    @Test fun zeroCalendarUnitsPreserveTheLaterOverlapInstant() {
        val fmt = JSONObject("""{"units":["months","hours"],"decimal":false}""")
        assertEquals("0 months, 1 hour left", CountdownText.remaining(
            instant("2027-11-07T07:30:00Z"), fmt, instant("2027-11-07T06:30:00Z"), newYork, Locale.US,
        ))
    }

    @Test fun calendarMonthsClampAndFixedDaysAccountForDst() {
        val fmt = JSONObject("""{"units":["months","days","hours"],"decimal":false}""")
        assertEquals("1 month, 2 days, 0 hours left", CountdownText.remaining(
            instant("2027-03-02T12:00:00Z"), fmt, instant("2027-01-31T12:00:00Z"), utc, Locale.US,
        ))
        val fixed = JSONObject("""{"units":["days","hours"],"decimal":false}""")
        assertEquals("0 days, 23 hours left", CountdownText.remaining(
            instant("2027-03-14T13:00:00Z"), fixed, instant("2027-03-13T14:00:00Z"), newYork, Locale.US,
        ))
    }
}
