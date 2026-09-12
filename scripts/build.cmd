@echo off
REM Caffeine Desktop build wrapper.
REM Bypasses PowerShell execution policy for this invocation only
REM (no system-wide change). Forwards all args, e.g.: build.cmd -NoBundle
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0build.ps1" %*
