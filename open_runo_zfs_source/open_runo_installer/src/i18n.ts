// 言語選択機能。英語(米国)をデフォルトとし、インストール時・インストール後の
// どちらでも切り替え可能(localStorageへ保存し、次回起動時も保持する)。
//
// これは「OpenRaidZインストーラー」(この`open-raid-z`リポジトリ専用の
// インストーラー)であり、"OpenRunoインストーラー"(OpenRuno全体のエコシステム
// 向けインストーラー、別リポジトリ)とは別物。デフォルト言語も両者で異なり、
// OpenRaidZインストーラーは英語既定・OpenRunoインストーラーは日本語既定。
//
// 対応言語(標準として要求されている10言語):
// アメリカ英語(既定) / イギリス英語 / 日本語 / イタリア語 / フランス語 /
// ドイツ語 / ロシア語 / ウクライナ語 / アラビア語(RTL) / ペルシャ語(RTL)

export type LangCode =
  | "ja"
  | "en-GB"
  | "en-US"
  | "it"
  | "fr"
  | "de"
  | "ru"
  | "uk"
  | "ar"
  | "fa";

export const RTL_LANGS: ReadonlySet<LangCode> = new Set(["ar", "fa"]);

export const LANG_NAMES: Record<LangCode, string> = {
  ja: "日本語",
  "en-GB": "English (UK)",
  "en-US": "English (US)",
  it: "Italiano",
  fr: "Français",
  de: "Deutsch",
  ru: "Русский",
  uk: "Українська",
  ar: "العربية",
  fa: "فارسی",
};

const ja = {
  app_title: "OpenRaidZ インストーラー",
  language_label: "言語",
  section_hardware: "ハードウェア構成",
  accelerator_label: "アクセラレータ",
  disks_label: "検出されたディスク",
  no_disks_admin_warning: "ディスクが検出されませんでした。管理者として実行してください。",
  refresh_button: "再スキャン",
  section_advice: "おすすめの構成",
  section_raid: "RAIDプールの初期化(プレビュー)",
  raid_level_label: "RAIDレベル",
  disk_count_label: "使用するディスク数",
  mirror_width_label: "ミラー幅(RAID10のみ)",
  dataset_name_label: "データセット名",
  init_button: "プレビュー実行",
  result_label: "結果",
  error_label: "エラー",
  loading: "読み込み中...",
  section_apply: "実ディスクへの適用",
  apply_warning: "警告: 選択したディスクの既存データは全て消去されます。この操作は元に戻せません。",
  confirm_data_loss_label: "選択したディスクの既存データが完全に消去されることを理解しました",
  apply_button: "実ディスクへ適用",
};

// 日本語辞書のキー集合を「唯一の正」とし、他言語の辞書型をこれに厳密化する。
// `Dict = Record<string, string>`のような緩い型だと、ある言語で翻訳キーが
// 1つ抜けていてもTypeScriptのコンパイルは通ってしまい(実行時に静かに
// 日本語やキー名そのものへフォールバックするだけになる)、キーの抜け漏れが
// レビューでしか発見できなかった。`Record<TranslationKey, string>`にする
// ことで、他言語の辞書にキーの過不足があれば`tsc`がコンパイルエラーとして
// 検出できるようにする。
type TranslationKey = keyof typeof ja;
type Dict = Record<TranslationKey, string>;

const enGB: Dict = {
  app_title: "OpenRaidZ Installer",
  language_label: "Language",
  section_hardware: "Hardware Configuration",
  accelerator_label: "Accelerator",
  disks_label: "Detected Disks",
  no_disks_admin_warning: "No disks detected. Please run as Administrator.",
  refresh_button: "Rescan",
  section_advice: "Recommended Configuration",
  section_raid: "Initialise RAID Pool (Preview)",
  raid_level_label: "RAID Level",
  disk_count_label: "Number of Disks",
  mirror_width_label: "Mirror Width (RAID10 only)",
  dataset_name_label: "Dataset Name",
  init_button: "Run Preview",
  result_label: "Result",
  error_label: "Error",
  loading: "Loading...",
  section_apply: "Apply to Real Disks",
  apply_warning: "Warning: all existing data on the selected disks will be permanently erased. This cannot be undone.",
  confirm_data_loss_label: "I understand the existing data on the selected disks will be permanently erased",
  apply_button: "Apply to Real Disks",
};

const enUS: Dict = {
  ...enGB,
  section_raid: "Initialize RAID Pool (Preview)",
};

const it: Dict = {
  app_title: "Installer OpenRaidZ",
  language_label: "Lingua",
  section_hardware: "Configurazione hardware",
  accelerator_label: "Acceleratore",
  disks_label: "Dischi rilevati",
  no_disks_admin_warning: "Nessun disco rilevato. Eseguire come amministratore.",
  refresh_button: "Ripeti scansione",
  section_advice: "Configurazione consigliata",
  section_raid: "Inizializza pool RAID (anteprima)",
  raid_level_label: "Livello RAID",
  disk_count_label: "Numero di dischi",
  mirror_width_label: "Larghezza mirror (solo RAID10)",
  dataset_name_label: "Nome dataset",
  init_button: "Esegui anteprima",
  result_label: "Risultato",
  error_label: "Errore",
  loading: "Caricamento...",
  section_apply: "Applica ai dischi reali",
  apply_warning: "Attenzione: tutti i dati esistenti sui dischi selezionati verranno cancellati definitivamente. Questa operazione non può essere annullata.",
  confirm_data_loss_label: "Ho compreso che i dati esistenti sui dischi selezionati verranno cancellati definitivamente",
  apply_button: "Applica ai dischi reali",
};

const fr: Dict = {
  app_title: "Programme d'installation OpenRaidZ",
  language_label: "Langue",
  section_hardware: "Configuration matérielle",
  accelerator_label: "Accélérateur",
  disks_label: "Disques détectés",
  no_disks_admin_warning: "Aucun disque détecté. Veuillez exécuter en tant qu'administrateur.",
  refresh_button: "Rescanner",
  section_advice: "Configuration recommandée",
  section_raid: "Initialiser le pool RAID (aperçu)",
  raid_level_label: "Niveau RAID",
  disk_count_label: "Nombre de disques",
  mirror_width_label: "Largeur du miroir (RAID10 uniquement)",
  dataset_name_label: "Nom du jeu de données",
  init_button: "Lancer l'aperçu",
  result_label: "Résultat",
  error_label: "Erreur",
  loading: "Chargement...",
  section_apply: "Appliquer aux disques réels",
  apply_warning: "Avertissement : toutes les données existantes sur les disques sélectionnés seront définitivement effacées. Cette opération est irréversible.",
  confirm_data_loss_label: "Je comprends que les données existantes sur les disques sélectionnés seront définitivement effacées",
  apply_button: "Appliquer aux disques réels",
};

const de: Dict = {
  app_title: "OpenRaidZ-Installationsprogramm",
  language_label: "Sprache",
  section_hardware: "Hardwarekonfiguration",
  accelerator_label: "Beschleuniger",
  disks_label: "Erkannte Laufwerke",
  no_disks_admin_warning: "Keine Laufwerke erkannt. Bitte als Administrator ausführen.",
  refresh_button: "Neu scannen",
  section_advice: "Empfohlene Konfiguration",
  section_raid: "RAID-Pool initialisieren (Vorschau)",
  raid_level_label: "RAID-Stufe",
  disk_count_label: "Anzahl der Laufwerke",
  mirror_width_label: "Spiegelbreite (nur RAID10)",
  dataset_name_label: "Datensatzname",
  init_button: "Vorschau ausführen",
  result_label: "Ergebnis",
  error_label: "Fehler",
  loading: "Wird geladen...",
  section_apply: "Auf echte Datenträger anwenden",
  apply_warning: "Warnung: Alle vorhandenen Daten auf den ausgewählten Datenträgern werden endgültig gelöscht. Dieser Vorgang kann nicht rückgängig gemacht werden.",
  confirm_data_loss_label: "Ich verstehe, dass die vorhandenen Daten auf den ausgewählten Datenträgern endgültig gelöscht werden",
  apply_button: "Auf echte Datenträger anwenden",
};

const ru: Dict = {
  app_title: "Установщик OpenRaidZ",
  language_label: "Язык",
  section_hardware: "Конфигурация оборудования",
  accelerator_label: "Ускоритель",
  disks_label: "Обнаруженные диски",
  no_disks_admin_warning: "Диски не обнаружены. Запустите от имени администратора.",
  refresh_button: "Повторить сканирование",
  section_advice: "Рекомендуемая конфигурация",
  section_raid: "Инициализация пула RAID (предпросмотр)",
  raid_level_label: "Уровень RAID",
  disk_count_label: "Количество дисков",
  mirror_width_label: "Ширина зеркала (только RAID10)",
  dataset_name_label: "Имя набора данных",
  init_button: "Запустить предпросмотр",
  result_label: "Результат",
  error_label: "Ошибка",
  loading: "Загрузка...",
  section_apply: "Применить к реальным дискам",
  apply_warning: "Внимание: все существующие данные на выбранных дисках будут безвозвратно удалены. Это действие невозможно отменить.",
  confirm_data_loss_label: "Я понимаю, что существующие данные на выбранных дисках будут безвозвратно удалены",
  apply_button: "Применить к реальным дискам",
};

const uk: Dict = {
  app_title: "Інсталятор OpenRaidZ",
  language_label: "Мова",
  section_hardware: "Конфігурація обладнання",
  accelerator_label: "Прискорювач",
  disks_label: "Виявлені диски",
  no_disks_admin_warning: "Диски не виявлено. Запустіть від імені адміністратора.",
  refresh_button: "Повторити сканування",
  section_advice: "Рекомендована конфігурація",
  section_raid: "Ініціалізація пулу RAID (попередній перегляд)",
  raid_level_label: "Рівень RAID",
  disk_count_label: "Кількість дисків",
  mirror_width_label: "Ширина дзеркала (лише RAID10)",
  dataset_name_label: "Назва набору даних",
  init_button: "Запустити попередній перегляд",
  result_label: "Результат",
  error_label: "Помилка",
  loading: "Завантаження...",
  section_apply: "Застосувати до реальних дисків",
  apply_warning: "Попередження: усі наявні дані на вибраних дисках будуть остаточно видалені. Цю дію неможливо скасувати.",
  confirm_data_loss_label: "Я розумію, що наявні дані на вибраних дисках будуть остаточно видалені",
  apply_button: "Застосувати до реальних дисків",
};

const ar: Dict = {
  app_title: "برنامج تثبيت OpenRaidZ",
  language_label: "اللغة",
  section_hardware: "تهيئة العتاد",
  accelerator_label: "المسرّع",
  disks_label: "الأقراص المكتشفة",
  no_disks_admin_warning: "لم يتم اكتشاف أي أقراص. يرجى التشغيل كمسؤول.",
  refresh_button: "إعادة الفحص",
  section_advice: "التهيئة الموصى بها",
  section_raid: "تهيئة مجمّع RAID (معاينة)",
  raid_level_label: "مستوى RAID",
  disk_count_label: "عدد الأقراص",
  mirror_width_label: "عرض المرآة (RAID10 فقط)",
  dataset_name_label: "اسم مجموعة البيانات",
  init_button: "تشغيل المعاينة",
  result_label: "النتيجة",
  error_label: "خطأ",
  loading: "جارٍ التحميل...",
  section_apply: "التطبيق على الأقراص الحقيقية",
  apply_warning: "تحذير: سيتم مسح جميع البيانات الموجودة على الأقراص المحددة بشكل نهائي. لا يمكن التراجع عن هذا الإجراء.",
  confirm_data_loss_label: "أفهم أن البيانات الموجودة على الأقراص المحددة ستُمسح بشكل نهائي",
  apply_button: "التطبيق على الأقراص الحقيقية",
};

const fa: Dict = {
  app_title: "نصب‌کننده OpenRaidZ",
  language_label: "زبان",
  section_hardware: "پیکربندی سخت‌افزار",
  accelerator_label: "شتاب‌دهنده",
  disks_label: "دیسک‌های شناسایی‌شده",
  no_disks_admin_warning: "هیچ دیسکی شناسایی نشد. لطفاً به‌عنوان مدیر اجرا کنید.",
  refresh_button: "اسکن مجدد",
  section_advice: "پیکربندی پیشنهادی",
  section_raid: "راه‌اندازی استخر RAID (پیش‌نمایش)",
  raid_level_label: "سطح RAID",
  disk_count_label: "تعداد دیسک‌ها",
  mirror_width_label: "عرض آینه (فقط RAID10)",
  dataset_name_label: "نام مجموعه‌داده",
  init_button: "اجرای پیش‌نمایش",
  result_label: "نتیجه",
  error_label: "خطا",
  loading: "در حال بارگذاری...",
  section_apply: "اعمال روی دیسک‌های واقعی",
  apply_warning: "هشدار: تمام داده‌های موجود روی دیسک‌های انتخاب‌شده برای همیشه پاک خواهد شد. این عملیات قابل بازگشت نیست.",
  confirm_data_loss_label: "متوجه‌ام که داده‌های موجود روی دیسک‌های انتخاب‌شده برای همیشه پاک خواهد شد",
  apply_button: "اعمال روی دیسک‌های واقعی",
};

const DICTS: Record<LangCode, Dict> = {
  ja,
  "en-GB": enGB,
  "en-US": enUS,
  it,
  fr,
  de,
  ru,
  uk,
  ar,
  fa,
};

const STORAGE_KEY = "open_runo_installer-lang";
const DEFAULT_LANG: LangCode = "en-US";

export function getLanguage(): LangCode {
  const stored = localStorage.getItem(STORAGE_KEY);
  if (stored && stored in DICTS) {
    return stored as LangCode;
  }
  return DEFAULT_LANG;
}

export function setLanguage(lang: LangCode): void {
  localStorage.setItem(STORAGE_KEY, lang);
  applyDocumentDirection(lang);
}

export function applyDocumentDirection(lang: LangCode): void {
  document.documentElement.dir = RTL_LANGS.has(lang) ? "rtl" : "ltr";
  document.documentElement.lang = lang;
}

export function t(key: string, lang: LangCode = getLanguage()): string {
  // `key`はDOMの`data-i18n`属性等、外部由来の任意文字列(呼び出し側で
  // 静的にキーの妥当性を保証できない)。辞書自体の型(`Dict`)は
  // `TranslationKey`に厳密化しているため、ここでは意図的に
  // `Record<string, string | undefined>`として緩めて参照する。
  const dict = DICTS[lang] as Record<string, string | undefined>;
  const fallback = DICTS[DEFAULT_LANG] as Record<string, string | undefined>;
  return dict[key] ?? fallback[key] ?? key;
}

export function allLanguages(): LangCode[] {
  return Object.keys(DICTS) as LangCode[];
}
