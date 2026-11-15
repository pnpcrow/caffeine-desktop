import 'package:flutter/material.dart';

/// Warm paper + mocha theme, fully specified.
///
/// Every text style is declared explicitly (color, size, weight, and
/// `decoration: none`) so no text ever inherits an ambient or fallback
/// style. Button/popup/dialog surfaces that Flutter styles from the theme
/// are also pinned to the palette.
class AppTheme {
  static const bg0 = Color(0xFFF8F4EA);
  static const bg1 = Color(0xFFFFFCF6);
  static const bg2 = Color(0xFFF0E8DC);
  static const cardBorder = Color(0xFFDED4C5);
  static const accent = Color(0xFF79573E);
  static const accentDeep = Color(0xFF657381);
  static const text = Color(0xFF302C29);
  static const muted = Color(0xFF71685F);
  static const green = Color(0xFF476951);
  static const red = Color(0xFFA1463B);
  static const ink = Color(0xFFFFFCF6);

  static const _plain = TextDecoration.none;

  static TextStyle _t(
    double size,
    Color color,
    FontWeight weight, {
    double? height,
  }) {
    return TextStyle(
      color: color,
      fontSize: size,
      fontWeight: weight,
      height: height,
      decoration: _plain,
      decorationColor: color,
    );
  }

  static ThemeData light() {
    final scheme = const ColorScheme.light(
      primary: accent,
      onPrimary: ink,
      secondary: accentDeep,
      onSecondary: ink,
      surface: bg1,
      onSurface: text,
      surfaceContainerLowest: bg0,
      surfaceContainerLow: bg1,
      surfaceContainerHigh: bg2,
      error: red,
      onError: ink,
    );

    final typography = TextTheme(
      displayLarge: _t(28, text, FontWeight.w800),
      displayMedium: _t(24, text, FontWeight.w800),
      displaySmall: _t(21, text, FontWeight.w800),
      headlineLarge: _t(20, text, FontWeight.w700),
      headlineMedium: _t(18, text, FontWeight.w700),
      headlineSmall: _t(21, text, FontWeight.w800),
      titleLarge: _t(16, text, FontWeight.w700),
      titleMedium: _t(14, text, FontWeight.w700),
      titleSmall: _t(13, text, FontWeight.w600),
      bodyLarge: _t(14, text, FontWeight.w400, height: 1.5),
      bodyMedium: _t(13, text, FontWeight.w400, height: 1.5),
      bodySmall: _t(12, muted, FontWeight.w400, height: 1.55),
      labelLarge: _t(13, text, FontWeight.w700),
      labelMedium: _t(12.5, text, FontWeight.w600),
      labelSmall: _t(11, muted, FontWeight.w400, height: 1.5),
    );

    final radius10 = RoundedRectangleBorder(
      borderRadius: BorderRadius.circular(10),
    );
    const buttonText = TextStyle(
      fontFamily: 'Malgun Gothic',
      fontSize: 13,
      fontWeight: FontWeight.w700,
      decoration: TextDecoration.none,
    );

    return ThemeData(
      useMaterial3: true,
      fontFamily: 'Malgun Gothic',
      colorScheme: scheme,
      scaffoldBackgroundColor: Colors.transparent,
      canvasColor: bg1,
      cardColor: bg2,
      dividerColor: cardBorder,
      splashFactory: NoSplash.splashFactory,
      highlightColor: Colors.transparent,
      textTheme: typography,
      primaryTextTheme: typography,
      elevatedButtonTheme: ElevatedButtonThemeData(
        style: ElevatedButton.styleFrom(
          backgroundColor: accent,
          foregroundColor: ink,
          disabledBackgroundColor: accent.withValues(alpha: 0.35),
          disabledForegroundColor: ink.withValues(alpha: 0.5),
          textStyle: buttonText.copyWith(color: ink),
          shape: radius10,
          padding: const EdgeInsets.symmetric(vertical: 12),
        ),
      ),
      outlinedButtonTheme: OutlinedButtonThemeData(
        style: OutlinedButton.styleFrom(
          foregroundColor: text,
          disabledForegroundColor: muted,
          side: const BorderSide(color: cardBorder),
          textStyle: buttonText.copyWith(color: text),
          shape: radius10,
          padding: const EdgeInsets.symmetric(vertical: 12),
        ),
      ),
      textButtonTheme: TextButtonThemeData(
        style: TextButton.styleFrom(
          foregroundColor: muted,
          textStyle: buttonText.copyWith(color: muted),
          shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(8)),
        ),
      ),
      switchTheme: SwitchThemeData(
        overlayColor: WidgetStatePropertyAll(accent.withValues(alpha: 0.10)),
        thumbColor: WidgetStateProperty.resolveWith(
          (states) => states.contains(WidgetState.selected) ? accent : muted,
        ),
        trackColor: WidgetStateProperty.resolveWith(
          (states) => states.contains(WidgetState.selected)
              ? accent.withValues(alpha: 0.35)
              : text.withValues(alpha: 0.12),
        ),
        trackOutlineColor: WidgetStateProperty.all(
          cardBorder.withValues(alpha: 0.6),
        ),
      ),
      dropdownMenuTheme: const DropdownMenuThemeData(
        menuStyle: MenuStyle(backgroundColor: WidgetStatePropertyAll(bg2)),
      ),
      popupMenuTheme: const PopupMenuThemeData(
        color: bg2,
        textStyle: TextStyle(
          color: text,
          fontSize: 12.5,
          decoration: TextDecoration.none,
        ),
      ),
      dialogTheme: const DialogThemeData(
        backgroundColor: bg2,
        titleTextStyle: TextStyle(
          color: text,
          fontSize: 14,
          fontWeight: FontWeight.w700,
          decoration: TextDecoration.none,
        ),
        contentTextStyle: TextStyle(
          color: muted,
          fontSize: 12,
          decoration: TextDecoration.none,
        ),
      ),
      tooltipTheme: const TooltipThemeData(
        decoration: BoxDecoration(
          color: bg2,
          borderRadius: BorderRadius.all(Radius.circular(8)),
        ),
        textStyle: TextStyle(
          color: text,
          fontSize: 12,
          decoration: TextDecoration.none,
        ),
      ),
      snackBarTheme: const SnackBarThemeData(
        backgroundColor: bg2,
        contentTextStyle: TextStyle(
          color: text,
          fontSize: 12,
          decoration: TextDecoration.none,
        ),
      ),
      listTileTheme: const ListTileThemeData(
        textColor: text,
        titleTextStyle: TextStyle(
          color: text,
          fontSize: 13,
          decoration: TextDecoration.none,
        ),
        subtitleTextStyle: TextStyle(
          color: muted,
          fontSize: 12,
          decoration: TextDecoration.none,
        ),
      ),
      iconTheme: const IconThemeData(color: text, size: 20),
    );
  }

  /// Root surface of the frameless settings window. The container fills the
  /// whole window and the window background is transparent, so this radius
  /// is the *only* visible corner curve (no DWM frame rounding on top of it)
  /// and the pixels outside it are see-through.
  static BoxDecoration shell() {
    return BoxDecoration(
      gradient: const LinearGradient(
        begin: Alignment.topLeft,
        end: Alignment.bottomRight,
        colors: [bg0, bg1, Color(0xFFF1E9DC)],
      ),
      border: Border.all(color: cardBorder),
      borderRadius: BorderRadius.circular(18),
    );
  }

  static BoxDecoration cardDeco() {
    return BoxDecoration(
      color: bg1,
      border: Border.all(color: cardBorder),
      borderRadius: BorderRadius.circular(18),
    );
  }
}
