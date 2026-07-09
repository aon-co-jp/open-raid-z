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
  media_type: string;
}

interface AcceleratorInfo {
  kind: string;
  description: string;
  vendor: string;
}

interface OsCompatEntry {
  os: string;
  status: string;
  note: string;
}

interface SystemStatus {
  current_os: string;
  os_compatibility: OsCompatEntry[];
  accelerators: AcceleratorInfo[];
  disks: DiskInfo[];
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
  const accelEl = document.getElementById("accelerator_info")!;
  accelEl.textContent = t("loading");

  const [accelerator, disks, advice] = await Promise.all([
    invoke<AcceleratorInfo>("detect_accelerator"),
    invoke<DiskInfo[]>("list_physical_disks"),
    invoke<Advice[]>("get_disk_advice"),
  ]);

  accelEl.textContent = `${accelerator.kind}: ${accelerator.description}`;

  const diskListEl = document.getElementById("disk_list")!;
  diskListEl.innerHTML = "";
  for (const disk of disks) {
    const li = document.createElement("li");
    const gib = (disk.size_bytes / (1024 * 1024 * 1024)).toFixed(1);
    const label = document.createElement("label");
    const checkbox = document.createElement("input");
    checkbox.type = "checkbox";
    checkbox.className = "disk_select_checkbox";
    checkbox.dataset.path = disk.path;
    checkbox.dataset.sizeBytes = String(disk.size_bytes);
    checkbox.addEventListener("change", updateApplyButtonState);
    label.appendChild(checkbox);
    label.append(` ${disk.path} — ${gib} GiB`);
    li.appendChild(label);
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

  updateApplyButtonState();
}

function openSystemStatusPanel(): void {
  const panel = document.getElementById("system_status_panel")!;
  panel.hidden = false;
  void loadSystemStatus();
}

function closeSystemStatusPanel(): void {
  document.getElementById("system_status_panel")!.hidden = true;
}

async function loadSystemStatus(): Promise<void> {
  const status = await invoke<SystemStatus>("get_system_status");

  document.getElementById("system_status_current_os")!.textContent = status.current_os;

  const osListEl = document.getElementById("system_status_os_list")!;
  osListEl.innerHTML = "";
  for (const entry of status.os_compatibility) {
    const li = document.createElement("li");
    li.className = `status_${entry.status}`;
    const statusLabel = t(`status_${entry.status}`);
    li.innerHTML = `<strong>${entry.os}</strong>: ${statusLabel} — ${entry.note}`;
    osListEl.appendChild(li);
  }

  const gpuListEl = document.getElementById("system_status_gpu_list")!;
  gpuListEl.innerHTML = "";
  if (status.accelerators.length === 0) {
    const li = document.createElement("li");
    li.textContent = t("no_gpu_detected");
    gpuListEl.appendChild(li);
  } else {
    for (const accel of status.accelerators) {
      const li = document.createElement("li");
      li.textContent = `[${accel.vendor}] ${accel.kind}: ${accel.description}`;
      gpuListEl.appendChild(li);
    }
  }

  const storageListEl = document.getElementById("system_status_storage_list")!;
  storageListEl.innerHTML = "";
  if (status.disks.length === 0) {
    const li = document.createElement("li");
    li.textContent = t("no_storage_detected");
    storageListEl.appendChild(li);
  } else {
    for (const disk of status.disks) {
      const li = document.createElement("li");
      const gib = (disk.size_bytes / (1024 * 1024 * 1024)).toFixed(1);
      li.textContent = `[${disk.media_type}] ${disk.path} — ${gib} GiB`;
      storageListEl.appendChild(li);
    }
  }
}

function getSelectedDisks(): { path: string; size_bytes: number }[] {
  const checkboxes = document.querySelectorAll<HTMLInputElement>(".disk_select_checkbox:checked");
  return Array.from(checkboxes).map((cb) => ({
    path: cb.dataset.path!,
    size_bytes: Number(cb.dataset.sizeBytes),
  }));
}

function updateApplyButtonState(): void {
  const confirmed = (document.getElementById("confirm_data_loss") as HTMLInputElement).checked;
  const hasSelection = getSelectedDisks().length > 0;
  (document.getElementById("apply_button") as HTMLButtonElement).disabled = !(confirmed && hasSelection);
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

async function submitApplyForm(e: SubmitEvent): Promise<void> {
  e.preventDefault();
  const level = (document.getElementById("apply_raid_level") as HTMLSelectElement).value;
  const datasetName = (document.getElementById("apply_dataset_name") as HTMLInputElement).value;
  const confirmDataLoss = (document.getElementById("confirm_data_loss") as HTMLInputElement).checked;
  const disks = getSelectedDisks();
  const resultEl = document.getElementById("apply_result")!;
  resultEl.textContent = t("loading");

  try {
    const result = await invoke("init_zpool_apply", {
      req: { disks, level, dataset_name: datasetName, confirm_data_loss: confirmDataLoss },
    });
    resultEl.textContent = `${t("result_label")}: ${JSON.stringify(result)}`;
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
  document.getElementById("system_status_toggle")?.addEventListener("click", openSystemStatusPanel);
  document.getElementById("system_status_close")?.addEventListener("click", closeSystemStatusPanel);
  document.getElementById("raid_level")?.addEventListener("change", toggleMirrorWidthVisibility);
  document.getElementById("raid_form")?.addEventListener("submit", (e) => void submitRaidForm(e));
  document.getElementById("confirm_data_loss")?.addEventListener("change", updateApplyButtonState);
  document.getElementById("apply_form")?.addEventListener("submit", (e) => void submitApplyForm(e));

  void loadHardware();
});
