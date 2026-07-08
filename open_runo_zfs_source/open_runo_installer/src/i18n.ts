// 言語選択機能。日本語をデフォルトとし、インストール時・インストール後の
// どちらでも切り替え可能(localStorageへ保存し、次回起動時も保持する)。
//
// 対応言語(標準として要求されている9言語):
// 日本語(既定) / イギリス英語 / アメリカ英語 / イタリア語 / フランス語 /
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
  app_title: "OpenRuno インストーラー",
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
  app_title: "OpenRuno Installer",
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
};

const enUS: Dict = {
  ...enGB,
  section_raid: "Initialize RAID Pool (Preview)",
};

const it: Dict = {
  app_title: "Installer OpenRuno",
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
};

const fr: Dict = {
  app_title: "Programme d'installation OpenRuno",
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
};

const de: Dict = {
  app_title: "OpenRuno-Installationsprogramm",
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
};

const ru: Dict = {
  app_title: "Установщик OpenRuno",
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
};

const uk: Dict = {
  app_title: "Інсталятор OpenRuno",
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
};

const ar: Dict = {
  app_title: "برنامج تثبيت OpenRuno",
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
};

const fa: Dict = {
  app_title: "نصب‌کننده OpenRuno",
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
const DEFAULT_LANG: LangCode = "ja";

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
