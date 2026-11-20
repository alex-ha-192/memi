package app.memi.data

import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.booleanPreferencesKey
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.core.stringSetPreferencesKey
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.map

enum class Appearance { LIGHT, DARK, SYSTEM }

data class ThemeChoice(val scheme: String = "Bento", val appearance: Appearance = Appearance.SYSTEM)

/** Theme and other small settings, kept in the app's preferences file. */
class ThemePrefs(private val store: DataStore<Preferences>) {
    private val schemeKey = stringPreferencesKey("scheme")
    private val appearanceKey = stringPreferencesKey("appearance")

    val choice: Flow<ThemeChoice> = store.data.map { prefs ->
        ThemeChoice(
            scheme = prefs[schemeKey] ?: "Bento",
            appearance = prefs[appearanceKey]?.let { runCatching { Appearance.valueOf(it) }.getOrNull() }
                ?: Appearance.SYSTEM,
        )
    }

    suspend fun setScheme(name: String) = store.edit { it[schemeKey] = name }

    suspend fun setAppearance(appearance: Appearance) = store.edit { it[appearanceKey] = appearance.name }

    private val freshCollapsedKey = booleanPreferencesKey("freshCollapsed")
    /** Still fresh on Home is folded away. */
    val freshCollapsed: Flow<Boolean> = store.data.map { it[freshCollapsedKey] ?: false }
    suspend fun setFreshCollapsed(collapsed: Boolean) = store.edit { it[freshCollapsedKey] = collapsed }

    private val welcomedKey = booleanPreferencesKey("welcomed")
    /** The first-launch questions have been answered (or skipped, because this device already had people or sync). */
    val welcomed: Flow<Boolean> = store.data.map { it[welcomedKey] ?: false }
    suspend fun setWelcomed() = store.edit { it[welcomedKey] = true }

    private val remindersKey = stringSetPreferencesKey("birthdayReminders")
    /** Days before a birthday to be reminded (0, 1, 7). Empty means off, the default. */
    val birthdayReminders: Flow<Set<Int>> = store.data.map { p -> p[remindersKey].orEmpty().mapNotNull { it.toIntOrNull() }.toSet() }
    suspend fun setBirthdayReminders(days: Set<Int>) = store.edit { it[remindersKey] = days.map { d -> d.toString() }.toSet() }

    private val remindedKey = stringSetPreferencesKey("reminded")
    val reminded: Flow<Set<String>> = store.data.map { it[remindedKey].orEmpty() }
    suspend fun addReminded(keys: Collection<String>) = store.edit { p ->
        // Only recent ones matter; each key ends with its date and days before.
        p[remindedKey] = (p[remindedKey].orEmpty() + keys).sortedBy { it.substringAfter(':') }.takeLast(100).toSet()
    }
}
