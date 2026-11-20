package app.memi

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.viewModels
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import app.memi.ui.AppViewModel
import app.memi.ui.MemiRoot

class MainActivity : ComponentActivity() {
    private val app get() = application as MemiApp
    private val vm: AppViewModel by viewModels {
        viewModelFactory { initializer { AppViewModel(app.repository, app.themePrefs) } }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
        setContent { MemiRoot(vm, app.sync) }
    }

    override fun onStart() {
        super.onStart()
        app.sync.onForeground()
    }

    override fun onStop() {
        app.sync.onBackground()
        super.onStop()
    }
}
