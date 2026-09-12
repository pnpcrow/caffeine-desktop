import 'dart:async';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:window_manager/window_manager.dart';

import 'settings_page.dart';
import 'src/rust/api.dart' as core;
import 'src/rust/frb_generated.dart';
import 'theme.dart';
import 'tray.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  await RustLib.init();
  await windowManager.ensureInitialized();

  // Single instance: the loser focuses the winner's window and exits.
  if (!await core.ensureSingleInstance()) {
    await core.focusExistingWindow();
    exit(0);
  }

  await core.initCore();
  final settings = await core.getSettings();

  const options = WindowOptions(
    size: Size(480, 700),
    minimumSize: Size(440, 620),
    center: true,
    backgroundColor: Colors.transparent,
    skipTaskbar: false,
    title: 'Caffeine Desktop',
    titleBarStyle: TitleBarStyle.hidden,
  );
  await windowManager.waitUntilReadyToShow(options, () async {
    await windowManager.setPreventClose(true);
  });

  await trayController.init();
  await trayController.refresh(await core.getStatus());

  runApp(const CaffeineApp());

  // Show only after the first frame is built: the runner no longer
  // auto-shows on first frame (that would defeat settings.startMinimized),
  // and showing before the first frame flashes an empty window.
  WidgetsBinding.instance.addPostFrameCallback((_) {
    if (!settings.startMinimized) {
      windowManager.show();
    }
  });
}

class CaffeineApp extends StatelessWidget {
  const CaffeineApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      debugShowCheckedModeBanner: false,
      title: 'Caffeine Desktop',
      theme: AppTheme.dark(),
      home: const SettingsPage(),
    );
  }
}
