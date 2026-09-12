import 'package:flutter/material.dart';

/// Espresso + caramel theme, fully specified.
///
/// Every text style is declared explicitly (color, size, weight, and
/// `decoration: none`) so no text ever inherits an ambient or fallback
/// style. Button/popup/dialog surfaces that Flutter styles from the theme
/// are also pinned to the palette.
class AppTheme {
  static const bg0 = Color(0xFF140D08);
  static const bg1 = Color(0xFF1E140D);
  static const bg2 = Color(0xFF2A1D12);
  static const cardBorder = Color(0x38E8A33D);
  static const accent = Color(0xFFE8A33D);
  static const accentDeep = Color(0xFFC96F2B);
  static const cream = Color(0xFFF5EBDD);
  static const muted = Color(0xFFB9A68E);
  static const green = Color(0xFF8FCE7E);
  static const red = Color(0xFFE0685C);
  static const ink = Color(0xFF241305);

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

  static ThemeData dark() {
    final scheme = const ColorScheme.dark(
      primary: accent,
      onPrimary: ink,
      secondary: accentDeep,
      onSecondary: cream,
      surface: bg1,
      onSurface: cream,
      surfaceContainerLowest: bg0,
      surfaceContainerLow: bg1,
      surfaceContainerHigh: bg2,
      error: red,
      onError: cream,
    );

    final text = TextTheme(
      displayLarge: _t(28, cream, FontWeight.w800),
      displayMedium: _t(24, cream, FontWeight.w800),
      displaySmall: _t(21, cream, FontWeight.w800),
      headlineLarge: _t(20, cream, FontWeight.w700),
      headlineMedium: _t(18, cream, FontWeight.w700),
      headlineSmall: _t(21, cream, FontWeight.w800),
      titleLarge: _t(16, cream, FontWeight.w700),
      titleMedium: _t(14, cream, FontWeight.w700),
      titleSmall: _t(13, cream, FontWeight.w600),
      bodyLarge: _t(14, cream, FontWeight.w400, height: 1.5),
      bodyMedium: _t(13, cream, FontWeight.w400, height: 1.5),
      bodySmall: _t(12, muted, FontWeight.w400, height: 1.55),
      labelLarge: _t(13, cream, FontWeight.w700),
      labelMedium: _t(12.5, cream, FontWeight.w600),
      labelSmall: _t(11, muted, FontWeight.w400, height: 1.5),
    );

    final radius10 =
        RoundedRectangleBorder(borderRadius: BorderRadius.circular(10));
    const buttonText = TextStyle(
      fontSize: 13,
      fontWeight: FontWeight.w700,
      decoration: TextDecoration.none,
    );

    return ThemeData(
      useMaterial3: true,
      colorScheme: scheme,
      scaffoldBackgroundColor: Colors.transparent,
      canvasColor: bg1,
      cardColor: bg2,
      dividerColor: cardBorder,
      splashFactory: NoSplash.splashFactory,
      highlightColor: Colors.transparent,
      textTheme: text,
      primaryTextTheme: text,
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
          foregroundColor: cream,
          disabledForegroundColor: muted,
          side: const BorderSide(color: cardBorder),
          textStyle: buttonText.copyWith(color: cream),
          shape: radius10,
          padding: const EdgeInsets.symmetric(vertical: 12),
        ),
      ),
      textButtonTheme: TextButtonThemeData(
        style: TextButton.styleFrom(
          foregroundColor: muted,
          textStyle: buttonText.copyWith(color: muted),
          shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(8)),
        ),
      ),
      switchTheme: SwitchThemeData(
        thumbColor: WidgetStateProperty.resolveWith((states) =>
            states.contains(WidgetState.selected) ? accent : muted),
        trackColor: WidgetStateProperty.resolveWith((states) =>
            states.contains(WidgetState.selected)
                ? accent.withValues(alpha: 0.35)
                : cream.withValues(alpha: 0.12)),
        trackOutlineColor:
            WidgetStateProperty.all(cardBorder.withValues(alpha: 0.6)),
      ),
      dropdownMenuTheme: const DropdownMenuThemeData(
        menuStyle: MenuStyle(
          backgroundColor: WidgetStatePropertyAll(bg2),
        ),
      ),
      popupMenuTheme: const PopupMenuThemeData(
        color: bg2,
        textStyle: TextStyle(
          color: cream,
          fontSize: 12.5,
          decoration: TextDecoration.none,
        ),
      ),
      dialogTheme: const DialogThemeData(
        backgroundColor: bg2,
        titleTextStyle: TextStyle(
          color: cream,
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
          color: cream,
          fontSize: 12,
          decoration: TextDecoration.none,
        ),
      ),
      snackBarTheme: const SnackBarThemeData(
        backgroundColor: bg2,
        contentTextStyle: TextStyle(
          color: cream,
          fontSize: 12,
          decoration: TextDecoration.none,
        ),
      ),
      listTileTheme: const ListTileThemeData(
        textColor: cream,
        titleTextStyle: TextStyle(
          color: cream,
          fontSize: 13,
          decoration: TextDecoration.none,
        ),
        subtitleTextStyle: TextStyle(
          color: muted,
          fontSize: 12,
          decoration: TextDecoration.none,
        ),
      ),
      iconTheme: const IconThemeData(color: cream, size: 20),
    );
  }

  static BoxDecoration shell() {
    return BoxDecoration(
      gradient: const LinearGradient(
        begin: Alignment.topLeft,
        end: Alignment.bottomRight,
        colors: [Color(0xFF241610), bg1, Color(0xFF120B07)],
      ),
      border: Border.all(color: cardBorder),
      borderRadius: BorderRadius.circular(18),
      boxShadow: const [
        BoxShadow(color: Colors.black54, blurRadius: 24, offset: Offset(0, 12)),
      ],
    );
  }

  static BoxDecoration cardDeco() {
    return BoxDecoration(
      color: const Color.fromRGBO(245, 235, 221, 0.045),
      border: Border.all(color: cardBorder),
      borderRadius: BorderRadius.circular(14),
    );
  }
}
