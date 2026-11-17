import 'package:flutter/material.dart';

import 'theme.dart';

/// A cup of caffeine to keep the screen awake, with the reference character.
class CoffeeHero extends StatelessWidget {
  const CoffeeHero({super.key});

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.fromLTRB(20, 16, 4, 0),
      decoration: BoxDecoration(
        color: AppTheme.bg2,
        borderRadius: BorderRadius.circular(22),
        border: Border.all(color: AppTheme.cardBorder),
      ),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.center,
        children: [
          const Expanded(
            child: Padding(
              padding: EdgeInsets.only(bottom: 20),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                mainAxisSize: MainAxisSize.min,
                children: [
                  Text(
                    'KEEP YOUR SCREEN AWAKE',
                    style: TextStyle(
                      color: AppTheme.accent,
                      fontSize: 9,
                      letterSpacing: 1.5,
                      fontWeight: FontWeight.w700,
                    ),
                  ),
                  SizedBox(height: 12),
                  Text(
                    '화면은 깨어 있게.\n카페인 한 잔.',
                    style: TextStyle(
                      color: AppTheme.text,
                      fontSize: 22,
                      height: 1.4,
                      letterSpacing: -0.7,
                      fontWeight: FontWeight.w700,
                    ),
                  ),
                  SizedBox(height: 10),
                  Text(
                    '절전 방지를 켜고,\n필요한 동안 화면을 켜 두세요.',
                    style: TextStyle(
                      color: AppTheme.muted,
                      fontSize: 12,
                      height: 1.6,
                    ),
                  ),
                ],
              ),
            ),
          ),
          const SizedBox(width: 4),
          ClipRRect(
            borderRadius: const BorderRadius.only(
              bottomRight: Radius.circular(21),
            ),
            child: Image.asset(
              'assets/illustrations/coffee-companion.png',
              width: 174,
              height: 210,
              fit: BoxFit.cover,
              alignment: Alignment.topCenter,
              semanticLabel: '머그컵의 커피를 마시는 캐릭터',
              filterQuality: FilterQuality.high,
            ),
          ),
        ],
      ),
    );
  }
}
