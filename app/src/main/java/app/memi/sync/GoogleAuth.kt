package app.memi.sync

import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import com.google.android.gms.auth.GoogleAuthUtil
import com.google.android.gms.auth.api.identity.AuthorizationRequest
import com.google.android.gms.auth.api.identity.AuthorizationResult
import com.google.android.gms.auth.api.identity.Identity
import com.google.android.gms.common.api.Scope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/**
 * Asks Google (via the account already on the phone) for access to Memi's own hidden Drive
 * folder, and nothing else.
 */
object GoogleAuth {
    const val DRIVE_APPDATA = "https://www.googleapis.com/auth/drive.appdata"

    sealed interface Outcome {
        data class Token(val value: String) : Outcome
        /** The person needs to pick an account or approve access; launch this. */
        data class NeedsConsent(val intent: PendingIntent) : Outcome
    }

    suspend fun authorize(context: Context): Outcome {
        val request = AuthorizationRequest.builder().setRequestedScopes(listOf(Scope(DRIVE_APPDATA))).build()
        val result: AuthorizationResult = Identity.getAuthorizationClient(context).authorize(request).await()
        val pending = result.pendingIntent
        return if (result.hasResolution() && pending != null) Outcome.NeedsConsent(pending)
        else Outcome.Token(result.accessToken ?: error("Google didn't return access"))
    }

    fun tokenFromIntent(context: Context, data: Intent?): String? =
        runCatching { Identity.getAuthorizationClient(context).getAuthorizationResultFromIntent(data).accessToken }.getOrNull()

    /** Drops a token Google has expired, so the next [authorize] fetches a fresh one. */
    suspend fun forget(context: Context, token: String) = withContext(Dispatchers.IO) {
        runCatching { GoogleAuthUtil.clearToken(context, token) }
        Unit
    }
}
