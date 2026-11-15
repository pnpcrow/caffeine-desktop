import 'dart:io';
import 'dart:ui' as ui;

import 'package:caffeine_desktop/coffee_hero.dart';
import 'package:caffeine_desktop/theme.dart';
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  setUpAll(() async {
    // Widget tests normally use Ahem (square glyphs). Use the same Windows
    // font as the shipped UI for meaningful Korean layout/visual checks.
    final loader = FontLoader('Malgun Gothic');
    for (final name in ['malgun.ttf', 'malgunbd.ttf']) {
      final bytes = await File(
        '${Platform.environment['WINDIR']}\\Fonts\\$name',
      ).readAsBytes();
      loader.addFont(Future.value(ByteData.sublistView(bytes)));
    }
    await loader.load();
  });
  testWidgets('coffee hero fits minimum window and enlarged text', (
    tester,
  ) async {
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetDevicePixelRatio);
    addTearDown(tester.view.resetPhysicalSize);
    for (final scale in [1.0, 1.3]) {
      tester.view.physicalSize = const Size(480, 640);
      await tester.pumpWidget(
        MaterialApp(
          theme: AppTheme.light(),
          home: MediaQuery(
            data: MediaQueryData(textScaler: TextScaler.linear(scale)),
            child: const Scaffold(
              body: Padding(padding: EdgeInsets.all(16), child: CoffeeHero()),
            ),
          ),
        ),
      );
      await tester.runAsync(
        () => precacheImage(
          const AssetImage('assets/illustrations/coffee-companion.png'),
          tester.element(find.byType(CoffeeHero)),
        ),
      );
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
      expect(find.byType(Image), findsOneWidget);
    }
  });

  testWidgets('render coffee theme preview', (tester) async {
    tester.view.devicePixelRatio = 1;
    tester.view.physicalSize = const Size(540, 330);
    addTearDown(tester.view.resetDevicePixelRatio);
    addTearDown(tester.view.resetPhysicalSize);
    final key = GlobalKey();
    await tester.pumpWidget(
      MaterialApp(
        theme: AppTheme.light(),
        home: RepaintBoundary(
          key: key,
          child: Scaffold(
            backgroundColor: AppTheme.bg0,
            body: Padding(
              padding: const EdgeInsets.all(16),
              child: Column(
                children: [
                  const CoffeeHero(),
                  const SizedBox(height: 16),
                  ElevatedButton(
                    onPressed: () {},
                    child: const Text('지금 화면 가리기'),
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
    await tester.runAsync(
      () => precacheImage(
        const AssetImage('assets/illustrations/coffee-companion.png'),
        tester.element(find.byType(CoffeeHero)),
      ),
    );
    await tester.pumpAndSettle();
    expect(tester.takeException(), isNull);
    final boundary =
        key.currentContext!.findRenderObject()! as RenderRepaintBoundary;
    await tester.runAsync(() async {
      final image = await boundary.toImage(pixelRatio: 2);
      final png = await image.toByteData(format: ui.ImageByteFormat.png);
      final file = File('build/qa/coffee-theme.png');
      await file.parent.create(recursive: true);
      await file.writeAsBytes(png!.buffer.asUint8List());
      image.dispose();
    });
  });
}
