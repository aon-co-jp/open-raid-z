import { invoke } from "@tauri-apps/api/core";
import {
  allLanguages,
  applyDocumentDirection,
  getLanguage,
  LANG_NAMES,
  LangCode,
  setLanguage,
  t,
} from "./i18n";

interface DiskInfo {
  path: string;
  index: number;
  size_bytes: number;
}

interface AcceleratorInfo {
  kind: string;
  description: string;
}

interface Advice {
  severity: "Info" | "Suggestion" | "Warning";
  title: string;
  detail: string;
}

function renderStaticText(): void {
  document.querySelectorAll<HTMLElement>("[data-i18n]").forEach((el) => {
    const key = el.dataset.i18n;
    if (key) el.textContent = t(key);
  });
  const warning = document.getElementById("no_disks_warning");
  if (warning) warning.textContent = t("no_disks_admin_warning");
}

function renderLangSelect(): void {
  const select = document.getElementById("lang_select") as HTMLSelectElement;
  select.innerHTML = "";
  for (const lang of allLanguages()) {
    const opt = document.createElement("option");
    opt.value = lang;
    opt.textContent = LANG_NAMES[lang];
    select.appendChild(opt);
  }
  select.value = getLanguage();
  select.addEventListener("change", () => {
    setLanguage(select.value as LangCode);
    renderStaticText();
  });
}

async function loadHardware(): Promise<void> {
  const [accelerator, disks, advice] = await Promise.all([
    invoke<AcceleratorInfo>("detect_accelerator"),
    invoke<DiskInfo[]>("list_physical_disks"),
    invoke<Advice[]>("get_disk_advice"),
  ]);

  const accelEl = document.getElementById("accelerator_info")!;
  accelEl.textContent = `${accelerator.kind}: ${accelerator.description}`;

  const diskListEl = document.getElementById("disk_list")!;
  diskListEl.innerHTML = "";
  for (const disk of disks) {
    const li = document.createElement("li");
    const gib = (disk.size_bytes / (1024 * 1024 * 1024)).toFixed(1);
    li.textContent = `${disk.path} — ${gib} GiB`;
    diskListEl.appendChild(li);
  }
  const warningEl = document.getElementById("no_disks_warning")!;
  warningEl.hidden = disks.length > 0;

  const adviceListEl = document.getElementById("advice_list")!;
  adviceListEl.innerHTML = "";
  for (const item of advice) {
    const li = document.createElement("li");
    li.className = `advice_${item.severity.toLowerCase()}`;
    li.innerHTML = `<strong>${item.title}</strong>: ${item.detail}`;
    adviceListEl.appendChild(li);
  }
}

function toggleMirrorWidthVisibility(): void {
  const level = (document.getElementById("raid_level") as HTMLSelectElement).value;
  const field = document.getElementById("mirror_width_field")!;
  field.style.display = level === "Raid10" ? "" : "none";
}

async function submitRaidForm(e: SubmitEvent): Promise<void> {
  e.preventDefault();
  const level = (document.getElementById("raid_level") as HTMLSelectElement).value;
  const diskCount = Number((document.getElementById("disk_count") as HTMLInputElement).value);
  const datasetName = (document.getElementById("dataset_name") as HTMLInputElement).value;
  const resultEl = document.getElementById("raid_result")!;
  resultEl.textContent = t("loading");

  try {
    if (level === "Raid10") {
      const mirrorWidth = Number((document.getElementById("mirror_width") as HTMLInputElement).value);
      const result = await invoke("init_raid10_preview", {
        req: { disk_count: diskCount, mirror_width: mirrorWidth, dataset_name: datasetName },
      });
      resultEl.textContent = `${t("result_label")}: ${JSON.stringify(result)}`;
    } else {
      const result = await invoke("init_zpool_preview", {
        req: { disk_count: diskCount, level, dataset_name: datasetName },
      });
      resultEl.textContent = `${t("result_label")}: ${JSON.stringify(result)}`;
    }
  } catch (err) {
    resultEl.textContent = `${t("error_label")}: ${err}`;
  }
}

window.addEventListener("DOMContentLoaded", () => {
  applyDocumentDirection(getLanguage());
  renderLangSelect();
  renderStaticText();
  toggleMirrorWidthVisibility();

  document.getElementById("refresh_button")?.addEventListener("click", () => void loadHardware());
  document.getElementById("raid_level")?.addEventListener("change", toggleMirrorWidthVisibility);
  document.getElementById("raid_form")?.addEventListener("submit", (e) => void submitRaidForm(e));

  void loadHardware();
});
