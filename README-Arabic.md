# open-raid-z (العربية)

**تطبيق بلغة Rust لمجمّع تخزين RAID-Z/Z2/Z3 حقيقي وقابل للتركيب
(mount).** يعيد تنفيذ أفكار تصميم ZFS/OpenZFS — التوزيع الشريطي مع
تعادلية موزّعة، ومجاميع اختبارية ذاتية الإصلاح، والنسخ عند الكتابة
(Copy-on-Write)، واللقطات/الاستنساخ — **من الصفر بلغة Rust، دون أي
اعتماد على OpenZFS نفسه**. تُنشئ أداة سطر الأوامر `orzctl` المجمّعات
**وتُركّبها فعليًا** على Windows (عبر WinFsp) وLinux/macOS/Android
(عبر FUSE).

> [الـ README الجذري](README.md) / [日本語](README-Japan.md) / [English](README-English.md) /
> [中文](README-Chinese.md) / [한국어](README-Korea.md) / [Español](README-Spain.md) /
> [Français](README-France.md) / [Deutsch](README-Germany.md) / [Italiano](README-Italy.md) /
> [Русский](README-Russia.md)

## تنبيه مهم

يستخدم open-raid-z **تنسيق قرص خاصًا به** (تخطيط CoW/striping على طراز
ZFS) و**غير متوافق على مستوى القرص مع ZFS الحقيقي** (لا يوجد uberblock
ولا ZIL وما إلى ذلك). الانتقال من ZFS/NTFS/ext4/أي RAID آخر موجود يعني
دائمًا "① القراءة من التنسيق الحالي ← ② نسخ الملفات بشكل عادي إلى مجمّع
open-raid-z". راجع [MIGRATION.md](MIGRATION.md).

## بنية مساحة العمل (3 حزم crate + مكوّنات مساعدة)

| المكوّن | الدور / الحالة |
|---|---|
| `open_raid_z_core` | المكتبة الأساسية: مستويات RAID (`Raid0`/`Raid1`/`Raid5`/`Raid6`≡`Z2`/`Z3`، التعداد `RaidLevel` في `vdev.rs`)، مجاميع اختبارية sha2، النسخ عند الكتابة، اللقطات/الاستنساخ، محاكاة ACL، التشغيل البيني مع FAT32/exFAT (`foreign_fs`، قراءة وكتابة)، التركيب الفعلي (WinFsp على Windows، FUSE على Linux/macOS/Android)، وملف `orzctl` التنفيذي لسطر الأوامر |
| `zfs_accel_hlsl` | يسرّع عبر GPU حساب التعادلية في حقل غالوا لـ RAID-Z/Z2/Z3 باستخدام تظليل HLSL + D3D12/DirectML. عند تعطيل ميزة `gpu_accel`، يتراجع إلى تنفيذ CPU خالص بلغة Rust (مفيد لبيئات CI بدون WinFsp/dxc) |
| `open_runo_installer_core` | منطق مستقل عن نظام التشغيل لاكتشاف الأقراص وتقديم مشورة تكوين zpool والمعاينة؛ فُصل عمدًا كحزمة مستقلة عن Tauri لتجنّب قيود edition2024 التي قد يفرضها Tauri |
| `open_runo_installer` (واجهة Tauri الرسومية) | تطبيق سطح مكتب مبني بـ Tauri 2 + TypeScript يستخدم `installer_core`. **هذا هو المكان الوحيد في كامل هذا النظام البيئي الذي يعتمد مباشرة على حزمة Tauri** (بمعزل عن سياسة مستودعات النظام البيئي للويب القاضية بإعادة تنفيذ Tauri من الصفر) |
| `wdk_driver/orzflt` | هيكل أدنى لمشغّل وضع النواة في Windows (WDF/KMDF 1.35). تم التحقق فقط من بناء التحميل/إلغاء التحميل؛ **اختبارات التحميل الفعلية مؤجَّلة عمدًا إلى جهاز افتراضي معزول** — مرحلة مبكرة |
| `third_party/fuser-0.17.0-android-patch` | نسخة مُعدَّلة (fork) من حزمة `fuser` تتيح بناءً خالصًا بلغة Rust لنظام Android. تُجمَّع تقاطعيًا إلى arm64-v8a عبر `cargo ndk`؛ لم يتم التحقق منها بعد على جهاز حقيقي |

## سطر أوامر `orzctl`

```sh
# إنشاء مجمّع Z2 عبر 6 أقراص
orzctl create --level z2 --chunk-size 4096 --stripes 100000 --dataset tank \
  /dev/sdb /dev/sdc /dev/sdd /dev/sde /dev/sdf /dev/sdg

# تركيبه فعليًا (يبقى في المقدمة)
orzctl mount --level z2 --chunk-size 4096 --stripes 100000 --mountpoint /mnt/tank \
  /dev/sdb /dev/sdc /dev/sdd /dev/sde /dev/sdf /dev/sdg

# قراءة/كتابة وحدة تخزين FAT32/exFAT موجودة (مساعدة للترحيل)
orzctl foreign ls /dev/sdb1
orzctl foreign --format exfat cat /dev/sdc1 /video.mp4 ./video.mp4
```

مستويات RAID المدعومة: `Raid0` / `Raid1` (مرآة) / `Raid5` / `Raid6`
(مطابق لـ `Z2`) / `Z2` / `Z3`. يتوفر RAID10 بشكل منفصل كحزمة من مجموعات
المرايا `Raid1` (`raid10.rs`).

## البناء والاختبار (مقاسة فعليًا)

```sh
cd open_runo_zfs_source/open_raid_z_core
cargo test --no-default-features
```

هذا بناء احتياطي يعتمد على CPU ولا يتطلب SDK الخاص بـ WinFsp ولا `dxc`
ولا SDK الخاص بـ Windows. تم القياس بتاريخ 2026-07-11:

| الحزمة | نجح | فشل |
|---|---|---|
| `open_raid_z_core` (`--no-default-features`) | 101 | 0 |
| `zfs_accel_hlsl` (`--no-default-features`، احتياطي CPU) | 32 | 0 |
| `open_runo_installer_core` | 30 | 0 |
| **الإجمالي** | **163** | **0** |

مجموعة ميزات `default` (`winfsp_backend` + `gpu_accel`، التركيب الفعلي +
الحوسبة الفعلية عبر GPU) تتطلب جهاز Windows حقيقيًا مع SDK الخاص بـ
WinFsp و`dxc`، ويجب التحقق منها بشكل منفصل.

## التوثيق

- [MIGRATION.md](MIGRATION.md) — الترحيل من ZFS/NTFS/ext4/أي RAID آخر
- [PORTING.md](PORTING.md) — دليل من صفحة واحدة لاعتماده في مشروع آخر
- [CLAUDE.md](CLAUDE.md) — قواعد التطوير / حزمة التقنيات (المرجع الأساسي لهذا النظام البيئي)
- [CHAT_HANDOFF.md](CHAT_HANDOFF.md) — تاريخ التطوير / ملاحظات التسليم

## الترخيص

MPL-2.0.
