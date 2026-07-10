@echo off
setlocal

set WDK_VER=10.0.26100.0
set WDK_ROOT=C:\Program Files (x86)\Windows Kits\10
set VC_VARS=C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat

call "%VC_VARS%"
if errorlevel 1 exit /b 1

set INCLUDE=%WDK_ROOT%\Include\%WDK_VER%\km;%WDK_ROOT%\Include\%WDK_VER%\km\crt;%WDK_ROOT%\Include\%WDK_VER%\shared;%WDK_ROOT%\Include\wdf\kmdf\1.35;%INCLUDE%
set LIB=%WDK_ROOT%\Lib\%WDK_VER%\km\x64;%WDK_ROOT%\Lib\wdf\kmdf\x64\1.35;%LIB%

cl.exe /c /kernel /Zi /Od /W4 /D_AMD64_ /DAMD64 /DKMDF_MAJOR_VERSION=1 /DKMDF_MINOR_VERSION=35 orzflt\driver.c /Foorzflt\driver.obj
if errorlevel 1 exit /b 1

link.exe /DRIVER /SUBSYSTEM:NATIVE /ENTRY:DriverEntry /NODEFAULTLIB /OUT:orzflt\orzflt.sys ^
    orzflt\driver.obj ^
    BufferOverflowFastFailK.lib ntoskrnl.lib hal.lib WdfLdr.lib WdfDriverEntry.lib
if errorlevel 1 exit /b 1

echo BUILD OK
endlocal
