import { useEffect, useState } from "react";
import { api } from "../api";
import type { DriveInfo, HardwareInfo, RuntimeInfo, SpaceRequirement } from "../types";
import { formatBytes } from "../types";
import clsx from "clsx";

export function Dashboard({
  drives,
  hardware,
  runtime,
  modelLabel,
  selectedDrive,
  onSelectDrive,
  onLoaded,
  onRemoved,
  onRefreshDrives,
}: {
  drives: DriveInfo[];
  hardware: HardwareInfo;
  runtime: RuntimeInfo;
  modelLabel: string;
  selectedDrive: string | null;
  onSelectDrive: (letter: string | null) => void;
  onLoaded: (rt: RuntimeInfo) => void;
  onRemoved: (rt: RuntimeInfo) => void;
  onRefreshDrives: () => void;
}) {
  const [space, setSpace] = useState<SpaceRequirement | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [confirmRemove, setConfirmRemove] = useState(false);
  const [liveRuntime, setLiveRuntime] = useState(runtime);
  const [tracking, setTracking] = useState(false);

  const hostDrives = drives.filter(
    (d) => d.isReady && d.driveType === "Fixed" && !d.isRemovable,
  );

  useEffect(() => {
    if (!tracking) setLiveRuntime(runtime);
  }, [runtime, tracking]);

  useEffect(() => {
    if (!selectedDrive) {
      setSpace(null);
      return;
    }
    api.getSpaceRequirement(selectedDrive).then(setSpace).catch((e) => setError(String(e)));
  }, [selectedDrive, runtime.status]);

  useEffect(() => {
    if (!selectedDrive || drives.length === 0) return;
    const allowed = drives.some(
      (d) =>
        d.letter === selectedDrive &&
        d.isReady &&
        d.driveType === "Fixed" &&
        !d.isRemovable,
    );
    if (!allowed) onSelectDrive(null);
  }, [selectedDrive, drives, onSelectDrive]);

  useEffect(() => {
    if (!tracking) return;
    let cancelled = false;
    const pull = () => {
      api
        .getRuntime()
        .then((rt) => {
          if (!cancelled) setLiveRuntime(rt);
        })
        .catch(() => undefined);
    };
    pull();
    const id = setInterval(pull, 400);
    return () => {
      cancelled = true;
      clearInterval(id);
    };
  }, [tracking]);

  const canLoad =
    !!selectedDrive &&
    !!space &&
    space.enoughSpace &&
    !space.missingModel &&
    !space.missingRuntime &&
    liveRuntime.status !== "loading" &&
    liveRuntime.status !== "running" &&
    liveRuntime.status !== "removing" &&
    !busy;

  async function handleLoad() {
    if (!selectedDrive) return;
    setBusy(true);
    setTracking(true);
    setError(null);
    setLiveRuntime((prev) => ({
      ...prev,
      status: "loading",
      progressPercent: 0,
      progressMessage: "Preparing...",
      error: null,
    }));
    try {
      const rt = await api.loadModel(selectedDrive);
      setLiveRuntime(rt);
      onLoaded(rt);
    } catch (e) {
      setError(String(e));
      const rt = await api.getRuntime();
      setLiveRuntime(rt);
    } finally {
      setTracking(false);
      setBusy(false);
    }
  }

  async function handleRemove() {
    setConfirmRemove(false);
    setBusy(true);
    setTracking(true);
    setError(null);
    setLiveRuntime((prev) => ({
      ...prev,
      status: "removing",
      progressPercent: 10,
      progressMessage: "Stopping Ollama...",
    }));
    try {
      const rt = await api.removeModel();
      setLiveRuntime(rt);
      onRemoved(rt);
    } catch (e) {
      setError(String(e));
    } finally {
      setTracking(false);
      setBusy(false);
    }
  }

  return (
    <div className="h-full overflow-y-auto p-8">
      <div className="max-w-4xl mx-auto space-y-8">
        <header>
          <h1 className="text-3xl font-semibold tracking-tight">PortableLLM</h1>
          <p className="text-[var(--text-muted)] mt-2">
            Select a host drive, load the model temporarily, chat locally, then remove everything.
          </p>
        </header>

        <section className="grid md:grid-cols-2 gap-4">
          <Card title="Model">
            <div className="text-xl">{modelLabel}</div>
            <div className="text-sm text-[var(--text-muted)] mt-1">
              {space ? formatBytes(space.modelBytes + space.runtimeBytes) : "-"} staged footprint
            </div>
          </Card>
          <Card title="Status">
            <div className="text-xl capitalize">
              {liveRuntime.status === "notLoaded" ? "Not Loaded" : liveRuntime.status}
            </div>
            {liveRuntime.targetDrive ? (
              <div className="text-sm text-[var(--text-muted)] mt-1">
                Target {liveRuntime.targetDrive} · {liveRuntime.stagingPath}
              </div>
            ) : (
              <div className="text-sm text-[var(--text-muted)] mt-1">No host installation</div>
            )}
          </Card>
        </section>

        <section className="grid md:grid-cols-2 gap-4">
          <Card title="Hardware">
            <InfoRow label="CPU" value={hardware.cpu} />
            <InfoRow label="RAM" value={formatBytes(hardware.ramBytes)} />
            <InfoRow label="GPU" value={hardware.gpu} />
            <InfoRow
              label="VRAM"
              value={hardware.vramBytes ? formatBytes(hardware.vramBytes) : "Unknown"}
            />
            <p className="text-sm text-[var(--text-muted)] mt-3">{hardware.note}</p>
          </Card>
          <Card title="Required Space">
            <InfoRow
              label="Required"
              value={space ? formatBytes(space.requiredBytes) : "Select a drive"}
            />
            <InfoRow
              label="Available"
              value={space ? formatBytes(space.availableBytes) : "-"}
            />
            {space?.missingRuntime ? (
              <p className="text-[var(--danger)] text-sm mt-3">
                Missing portable Ollama runtime in portable/runtime
              </p>
            ) : null}
            {space?.missingModel ? (
              <p className="text-[var(--danger)] text-sm mt-3">
                Missing model files in portable/model
              </p>
            ) : null}
            {space && !space.missingModel && !space.missingRuntime ? (
              <p
                className={clsx(
                  "text-sm mt-3",
                  space.enoughSpace ? "text-[var(--success)]" : "text-[var(--danger)]",
                )}
              >
                {space.enoughSpace ? "✓ Enough space" : "Insufficient space"}
              </p>
            ) : null}
          </Card>
        </section>

        <section>
          <div className="flex items-center justify-between mb-3">
            <h2 className="text-lg font-medium">Target Drive</h2>
            <button
              className="text-sm text-[var(--accent)]"
              onClick={onRefreshDrives}
            >
              Refresh
            </button>
          </div>
          <div className="grid gap-2">
            {hostDrives.length === 0 ? (
              <p className="text-sm text-[var(--text-muted)]">
                No fixed drives found. Removable drives, including the flash drive, are hidden.
              </p>
            ) : null}
            {hostDrives.map((d) => (
                <button
                  key={d.letter}
                  onClick={() => onSelectDrive(d.letter)}
                  className={clsx(
                    "text-left rounded-xl border px-4 py-3 transition",
                    selectedDrive === d.letter
                      ? "border-[var(--accent)] bg-[#152033]"
                      : "border-[var(--border)] hover:border-[#3b4b60] bg-[var(--bg-elevated)]/60",
                  )}
                >
                  <div className="flex justify-between gap-4">
                    <div>
                      <div className="font-medium">
                        {d.letter} {d.name ? `· ${d.name}` : ""}
                      </div>
                      <div className="text-sm text-[var(--text-muted)]">
                        {d.mediaType} · {d.driveType}
                      </div>
                    </div>
                    <div className="text-right text-sm text-[var(--text-muted)]">
                      <div>{formatBytes(d.freeBytes)} free</div>
                      <div>{formatBytes(d.totalBytes)} total</div>
                    </div>
                  </div>
                </button>
              ))}
          </div>
        </section>

        {(tracking ||
          liveRuntime.status === "loading" ||
          liveRuntime.status === "removing") && (
          <section className="rounded-xl border border-[var(--border)] bg-[var(--bg-elevated)] p-4">
            <div className="flex justify-between text-sm mb-2 gap-3">
              <span>{liveRuntime.progressMessage || "Working..."}</span>
              <span className="text-[var(--text-muted)]">{liveRuntime.progressPercent}%</span>
            </div>
            <div className="h-2 rounded bg-[#0b1016] overflow-hidden">
              <div
                className="h-full bg-[var(--accent)] transition-all"
                style={{ width: `${Math.min(100, liveRuntime.progressPercent)}%` }}
              />
            </div>
          </section>
        )}

        {error ? (
          <div className="rounded-xl border border-[var(--danger)]/40 bg-[#2a1518] px-4 py-3 text-sm whitespace-pre-wrap">
            {error}
          </div>
        ) : null}

        <div className="flex flex-wrap gap-3 pt-2">
          <button
            disabled={!canLoad}
            onClick={handleLoad}
            className={clsx(
              "px-5 py-2.5 rounded-xl font-medium",
              canLoad
                ? "bg-[var(--accent)] hover:bg-[var(--accent-dim)] text-white"
                : "bg-[#243041] text-[#6b7c90] cursor-not-allowed",
            )}
          >
            Load Model
          </button>
          <button
            disabled={liveRuntime.status !== "running" || busy}
            onClick={() => setConfirmRemove(true)}
            className="px-5 py-2.5 rounded-xl border border-[var(--border)] hover:border-[var(--danger)] disabled:opacity-40"
          >
            Remove Model
          </button>
        </div>

        {confirmRemove ? (
          <div className="fixed inset-0 bg-black/60 flex items-center justify-center p-4 z-50">
            <div className="bg-[var(--bg-elevated)] border border-[var(--border)] rounded-2xl p-6 max-w-md w-full space-y-4">
              <h3 className="text-lg font-medium">Remove host installation?</h3>
              <p className="text-sm text-[var(--text-muted)]">
                This stops Ollama and deletes only{" "}
                <code className="text-[var(--warning)]">
                  {liveRuntime.stagingPath ?? "<drive>:\\PortableLLM"}
                </code>
                . The USB source stays intact.
              </p>
              <div className="flex gap-3 justify-end">
                <button
                  className="px-4 py-2 rounded-lg border border-[var(--border)]"
                  onClick={() => setConfirmRemove(false)}
                >
                  Cancel
                </button>
                <button
                  className="px-4 py-2 rounded-lg bg-[var(--danger)] text-white"
                  onClick={handleRemove}
                >
                  Remove
                </button>
              </div>
            </div>
          </div>
        ) : null}
      </div>
    </div>
  );
}

function Card({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="rounded-2xl border border-[var(--border)] bg-[var(--bg-elevated)]/70 p-4">
      <div className="text-xs uppercase tracking-wide text-[var(--text-muted)] mb-2">{title}</div>
      {children}
    </div>
  );
}

function InfoRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex justify-between gap-4 py-1 text-sm">
      <span className="text-[var(--text-muted)]">{label}</span>
      <span className="text-right">{value}</span>
    </div>
  );
}
