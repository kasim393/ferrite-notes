package com.ferritenotes.notes

import android.os.Bundle
import android.view.View
import androidx.activity.enableEdgeToEdge
import androidx.core.graphics.Insets
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    keepAboveKeyboard(findViewById(android.R.id.content))
  }

  /**
   * Edge-to-edge apps aren't resized for the keyboard, so the editor would end
   * up behind it. Shrink the content by the keyboard's height instead; the web
   * page then lays out in the space that's left. While the keyboard is up, the
   * gesture bar is behind it, so the page gets no bottom inset for it.
   */
  private fun keepAboveKeyboard(content: View) {
    ViewCompat.setOnApplyWindowInsetsListener(content) { view, insets ->
      val ime = insets.getInsets(WindowInsetsCompat.Type.ime()).bottom
      view.setPadding(0, 0, 0, ime)
      if (ime == 0) {
        insets
      } else {
        val bars = insets.getInsets(WindowInsetsCompat.Type.systemBars())
        WindowInsetsCompat.Builder(insets)
          .setInsets(WindowInsetsCompat.Type.systemBars(), Insets.of(bars.left, bars.top, bars.right, 0))
          .build()
      }
    }
  }
}
