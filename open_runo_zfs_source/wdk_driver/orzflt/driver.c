/*
 * orzflt - open-raid-z 起動ドライバの最小スケルトン(KMDF, "Hello World"段階)。
 *
 * 目的: WindowsをRAID-Zプール上から起動できるようにする、という長期目標
 * (CHAT_HANDOFF.md 追記1〜8参照)へ向けた最初の一歩。カーネルドライバの
 * バグはブート不能・BSODに直結するため、ここでは意図的にスコープを
 * 「WDFドライバオブジェクトのロード/アンロードが確認できるだけの、
 * 実I/Oを一切行わない制御デバイス」に絞る。実際のRAID-Z読み書きロジックを
 * カーネル空間へ持ち込むのは、この骨格のロード確認が取れてから段階的に
 * 進める方針(テスト署名モード・隔離VMでの検証が前提)。
 */

#include <ntddk.h>
#include <wdf.h>

DRIVER_INITIALIZE DriverEntry;
EVT_WDF_DRIVER_DEVICE_ADD OrzfltEvtDeviceAdd;
EVT_WDF_OBJECT_CONTEXT_CLEANUP OrzfltEvtDriverContextCleanup;

_Use_decl_annotations_
NTSTATUS
OrzfltEvtDeviceAdd(
    _In_ WDFDRIVER Driver,
    _Inout_ PWDFDEVICE_INIT DeviceInit
    )
{
    NTSTATUS status;
    WDFDEVICE device;

    UNREFERENCED_PARAMETER(Driver);

    // 現段階ではI/Oを一切扱わない、素のWDFDEVICEを1つ作成するだけ。
    status = WdfDeviceCreate(&DeviceInit, WDF_NO_OBJECT_ATTRIBUTES, &device);
    if (!NT_SUCCESS(status)) {
        return status;
    }

    UNREFERENCED_PARAMETER(device);
    return STATUS_SUCCESS;
}

VOID
OrzfltEvtDriverContextCleanup(
    _In_ WDFOBJECT DriverObject
    )
{
    UNREFERENCED_PARAMETER(DriverObject);
}

NTSTATUS
DriverEntry(
    _In_ PDRIVER_OBJECT DriverObject,
    _In_ PUNICODE_STRING RegistryPath
    )
{
    NTSTATUS status;
    WDF_DRIVER_CONFIG config;
    WDF_OBJECT_ATTRIBUTES attributes;

    WDF_OBJECT_ATTRIBUTES_INIT(&attributes);
    attributes.EvtCleanupCallback = OrzfltEvtDriverContextCleanup;

    WDF_DRIVER_CONFIG_INIT(&config, OrzfltEvtDeviceAdd);

    status = WdfDriverCreate(
        DriverObject,
        RegistryPath,
        &attributes,
        &config,
        WDF_NO_HANDLE
        );

    return status;
}
