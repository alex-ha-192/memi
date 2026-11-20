package app.memi.data

import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.stringPreferencesKey
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.map

enum class Appearance { LIGHT, DARK, SYSTEM }

data class ThemeChoice(val scheme: String = "Bento", val appearance: Appearance = Appearance.SYSTEM)

/** Theme settings, kept in a small preferences file each app provides. */
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
}
