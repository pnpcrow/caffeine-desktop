@echo off
REM Caffeine Desktop dev wrapper.
REM Bypasses PowerShell execution policy for this invocation only
REM (no system-wide change).
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0dev.ps1" %*
