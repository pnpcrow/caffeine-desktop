import 'package:caffeine_desktop/src/rust/settings.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('generated enum shapes match the product spec', () {
    expect(UnlockKey.values.length, 4);
    expect(UnlockMouse.values.length, 4);
    expect(UnlockKey.values.byName('esc'), UnlockKey.esc);
    expect(UnlockMouse.values.byName('shake'), UnlockMouse.shake);
  });
}
