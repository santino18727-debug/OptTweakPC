import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

type Risk = "safe" | "read" | "moderate";

interface StartupEntry {
  programme: string;
  emplacement: string;
}

interface CleanupItem {
  nom: string;
  categorie: string; // "service" | "tache"
  etat: string; // "cible" | "modifie" | "absent" | "echec" | "restaure"
  detail: string;
  ancien?: string | null; // état précédent (avant/après)
}

interface CleanupResult {
  items: CleanupItem[];
  modifies: number;
  requires_reboot: boolean;
}

interface SystemInfo {
  os: string;
  cpu: string;
  ram_total_gb: number;
  ram_free_gb: number;
  power_plan: string;
  startup_count: number;
}

interface Status {
  kind: "success" | "error" | "info";
  text: string;
}

interface JournalEntry {
  time: string;
  label: string;
  kind: "success" | "error" | "info";
}

const RISK_META: Record<Risk, { label: string; cls: string }> = {
  safe: { label: "Sans risque", cls: "risk-safe" },
  read: { label: "Lecture seule", cls: "risk-read" },
  moderate: { label: "Modifie le système", cls: "risk-moderate" },
};

function RiskBadge({ risk }: { risk: Risk }) {
  const meta = RISK_META[risk];
  return <span className={`risk-badge ${meta.cls}`}>{meta.label}</span>;
}

const ETAT_CLS: Record<string, string> = {
  cible: "tag-target",
  modifie: "tag-done",
  absent: "tag-skip",
  echec: "tag-fail",
  restaure: "tag-read",
};

function App() {
  const [status, setStatus] = useState<Status | null>(null);
  const [startup, setStartup] = useState<StartupEntry[] | null>(null);
  const [preview, setPreview] = useState<CleanupResult | null>(null);
  const [cleanup, setCleanup] = useState<CleanupResult | null>(null);
  const [activeCommand, setActiveCommand] = useState<string | null>(null);
  const [isAdmin, setIsAdmin] = useState<boolean | null>(null);
  const [confirmOptimize, setConfirmOptimize] = useState(false);
  const [sysInfo, setSysInfo] = useState<SystemInfo | null>(null);
  const [hasSnapshot, setHasSnapshot] = useState(false);
  const [journal, setJournal] = useState<JournalEntry[]>([]);

  // Détecte les droits admin + contexte système au montage.
  useEffect(() => {
    invoke<boolean>("check_admin_rights").then(setIsAdmin).catch(() => setIsAdmin(null));
    invoke<SystemInfo>("get_system_info").then(setSysInfo).catch(() => setSysInfo(null));
    invoke<boolean>("has_startup_snapshot").then(setHasSnapshot).catch(() => setHasSnapshot(false));
  }, []);

  // Ajoute une entrée horodatée au journal de session (en mémoire, non persisté).
  function logJournal(label: string, kind: JournalEntry["kind"]) {
    const time = new Date().toLocaleTimeString("fr-CH");
    setJournal((j) => [{ time, label, kind }, ...j].slice(0, 20));
  }

  // Réinitialise tous les panneaux de résultat avant une nouvelle opération.
  function resetResults() {
    setStatus(null);
    setStartup(null);
    setPreview(null);
    setCleanup(null);
  }

  // Commande simple renvoyant un message texte (restore point, optimize).
  async function runMessageCommand(command: string, loadingText: string) {
    resetResults();
    setActiveCommand(command);
    setStatus({ kind: "info", text: loadingText });
    try {
      const response = await invoke<string>(command);
      setStatus({ kind: "success", text: response });
      logJournal(response.split("\n")[0], "success");
    } catch (error) {
      setStatus({ kind: "error", text: String(error) });
      logJournal(`${command} — échec`, "error");
    } finally {
      setActiveCommand(null);
    }
  }

  async function runAudit() {
    resetResults();
    setActiveCommand("audit_startup");
    setStatus({ kind: "info", text: "Audit du démarrage en cours…" });
    try {
      const entries = await invoke<StartupEntry[]>("audit_startup");
      setStartup(entries);
      setStatus({
        kind: "success",
        text:
          entries.length === 0
            ? "Aucun programme de démarrage tiers détecté."
            : `${entries.length} programme(s) au démarrage.`,
      });
    } catch (error) {
      setStatus({ kind: "error", text: String(error) });
    } finally {
      setActiveCommand(null);
    }
  }

  // Étape 1 du nettoyage : aperçu (dry-run), aucune modification.
  async function runPreview() {
    resetResults();
    setActiveCommand("preview_startup_cleanup");
    setStatus({ kind: "info", text: "Analyse de ce qui peut être nettoyé…" });
    try {
      const result = await invoke<CleanupResult>("preview_startup_cleanup");
      setPreview(result);
      const cibles = result.items.filter((i) => i.etat === "cible").length;
      setStatus({
        kind: cibles > 0 ? "info" : "success",
        text:
          cibles > 0
            ? `${cibles} élément(s) peuvent être allégés. Vérifiez puis appliquez.`
            : "Démarrage déjà optimisé : rien à nettoyer sur ce poste.",
      });
    } catch (error) {
      setStatus({ kind: "error", text: String(error) });
    } finally {
      setActiveCommand(null);
    }
  }

  // Étape 2 : application réelle, après que l'utilisateur a vu l'aperçu.
  async function applyCleanup() {
    setActiveCommand("clean_startup");
    setStatus({ kind: "info", text: "Nettoyage du démarrage en cours…" });
    try {
      const result = await invoke<CleanupResult>("clean_startup");
      setPreview(null);
      setCleanup(result);
      const ok =
        result.modifies > 0
          ? `${result.modifies} élément(s) modifié(s).` +
            (result.requires_reboot ? " Effet au prochain redémarrage." : "")
          : "Aucun changement appliqué.";
      setStatus({ kind: "success", text: ok });
      logJournal(`Nettoyage démarrage — ${ok}`, "success");
      if (result.modifies > 0) setHasSnapshot(true);
    } catch (error) {
      setStatus({ kind: "error", text: String(error) });
      logJournal(`Nettoyage démarrage — échec`, "error");
    } finally {
      setActiveCommand(null);
    }
  }

  // Annule le dernier nettoyage à partir du snapshot d'état sauvegardé.
  async function runRestore() {
    resetResults();
    setActiveCommand("restore_startup");
    setStatus({ kind: "info", text: "Restauration de l'état précédent…" });
    try {
      const result = await invoke<CleanupResult>("restore_startup");
      setCleanup(result);
      const ok = `${result.modifies} élément(s) restauré(s).`;
      setStatus({ kind: "success", text: ok });
      logJournal(`Restauration — ${ok}`, "success");
      setHasSnapshot(false);
    } catch (error) {
      setStatus({ kind: "error", text: String(error) });
      logJournal(`Restauration — échec`, "error");
    } finally {
      setActiveCommand(null);
    }
  }

  const busy = activeCommand !== null;

  return (
    <main className="container">
      <header className="app-header">
        <h1>OptTweakPC</h1>
        <p className="subtitle">
          Optimisez votre système de manière modulaire et sécurisée.
        </p>
      </header>

      {isAdmin === false && (
        <div className="admin-banner" role="alert">
          ⚠️ L'application n'a pas les droits administrateur. Les actions qui
          modifient le système échoueront. Relancez OptTweakPC en tant
          qu'administrateur.
        </div>
      )}

      {sysInfo && (
        <div className="dashboard" aria-label="Informations système">
          <div className="stat">
            <span className="stat-label">Plan d'alimentation</span>
            <span className="stat-value">{sysInfo.power_plan}</span>
          </div>
          <div className="stat">
            <span className="stat-label">Mémoire libre</span>
            <span className="stat-value">
              {sysInfo.ram_free_gb} / {sysInfo.ram_total_gb} Go
            </span>
          </div>
          <div className="stat">
            <span className="stat-label">Au démarrage</span>
            <span className="stat-value">{sysInfo.startup_count} programmes</span>
          </div>
          <div className="stat">
            <span className="stat-label">Processeur</span>
            <span className="stat-value stat-cpu" title={sysInfo.cpu}>
              {sysInfo.cpu}
            </span>
          </div>
        </div>
      )}

      <section className="card">
        <div className="card-head">
          <h2>1. Sécurité</h2>
          <RiskBadge risk="safe" />
        </div>
        <p>
          Crée un point de restauration Windows. À faire toujours en premier,
          avant toute modification — il permet de tout annuler.
        </p>
        <button
          className="btn btn-primary"
          onClick={() =>
            runMessageCommand(
              "create_restore_point",
              "Création du point de restauration en cours… (cela peut prendre jusqu'à une minute)"
            )
          }
          disabled={busy}
        >
          {activeCommand === "create_restore_point"
            ? "Création…"
            : "Créer un point de restauration"}
        </button>
      </section>

      <section className="card">
        <div className="card-head">
          <h2>2. Performances</h2>
          <RiskBadge risk="moderate" />
        </div>
        <p>
          Active le plan d'alimentation « Performances élevées » de Windows.
          Réversible à tout moment depuis les paramètres d'alimentation.
        </p>
        <button
          className="btn btn-primary"
          onClick={() => setConfirmOptimize(true)}
          disabled={busy}
        >
          {activeCommand === "optimize_performance"
            ? "Optimisation…"
            : "Optimiser les performances"}
        </button>
      </section>

      <section className="card">
        <div className="card-head">
          <h2>3. Démarrage</h2>
          <RiskBadge risk="moderate" />
        </div>
        <p>
          Auditez les programmes lancés au démarrage, puis allégez-le en
          désactivant les updaters et services de monitoring non essentiels (le
          Bureau à distance est préservé).
        </p>
        <div className="btn-row">
          <button
            className="btn btn-secondary"
            onClick={runAudit}
            disabled={busy}
          >
            {activeCommand === "audit_startup" ? "Audit…" : "Auditer le démarrage"}
          </button>
          <button
            className="btn btn-secondary"
            onClick={runPreview}
            disabled={busy}
          >
            {activeCommand === "preview_startup_cleanup"
              ? "Analyse…"
              : "Analyser le nettoyage"}
          </button>
          {hasSnapshot && (
            <button
              className="btn btn-secondary"
              onClick={runRestore}
              disabled={busy}
            >
              {activeCommand === "restore_startup"
                ? "Restauration…"
                : "↩ Annuler le nettoyage"}
            </button>
          )}
        </div>
      </section>

      {status && (
        <div
          className={`status-box ${status.kind}`}
          role="status"
          aria-live="polite"
        >
          <pre>{status.text}</pre>
        </div>
      )}

      {startup && startup.length > 0 && (
        <div className="result-panel">
          <h3>Programmes au démarrage</h3>
          <table className="data-table">
            <thead>
              <tr>
                <th>Programme</th>
                <th>Emplacement</th>
              </tr>
            </thead>
            <tbody>
              {startup.map((e, i) => (
                <tr key={i}>
                  <td>{e.programme}</td>
                  <td className="cell-mono">{e.emplacement}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {preview && (
        <div className="result-panel">
          <h3>Aperçu du nettoyage (aucune modification effectuée)</h3>
          <CleanupTable items={preview.items} />
          {preview.items.some((i) => i.etat === "cible") && (
            <div className="panel-actions">
              <button
                className="btn btn-warning"
                onClick={applyCleanup}
                disabled={busy}
              >
                {activeCommand === "clean_startup"
                  ? "Application…"
                  : "Appliquer le nettoyage"}
              </button>
            </div>
          )}
        </div>
      )}

      {cleanup && (
        <div className="result-panel">
          <h3>Résultat du nettoyage</h3>
          <CleanupTable items={cleanup.items} />
        </div>
      )}

      {journal.length > 0 && (
        <div className="result-panel">
          <h3>Journal de session</h3>
          <ul className="journal">
            {journal.map((j, i) => (
              <li key={i} className={`journal-${j.kind}`}>
                <span className="journal-time">{j.time}</span>
                <span>{j.label}</span>
              </li>
            ))}
          </ul>
        </div>
      )}

      {confirmOptimize && (
        <div
          className="modal-overlay"
          role="dialog"
          aria-modal="true"
          aria-label="Confirmation : optimiser les performances"
        >
          <div className="modal">
            <h3>Confirmer l'action</h3>
            <p>
              Le plan d'alimentation actif va être remplacé par « Performances
              élevées ». Continuer ?
            </p>
            <div className="modal-actions">
              <button
                className="btn btn-secondary"
                onClick={() => setConfirmOptimize(false)}
              >
                Annuler
              </button>
              <button
                className="btn btn-warning"
                onClick={() => {
                  setConfirmOptimize(false);
                  runMessageCommand(
                    "optimize_performance",
                    "Optimisation des performances en cours…"
                  );
                }}
              >
                Optimiser les performances
              </button>
            </div>
          </div>
        </div>
      )}
    </main>
  );
}

function CleanupTable({ items }: { items: CleanupItem[] }) {
  return (
    <table className="data-table">
      <thead>
        <tr>
          <th>Élément</th>
          <th>Type</th>
          <th>Avant</th>
          <th>État</th>
          <th>Détail</th>
        </tr>
      </thead>
      <tbody>
        {items.map((it, i) => (
          <tr key={i}>
            <td>{it.nom}</td>
            <td>{it.categorie}</td>
            <td className="cell-mono">{it.ancien ?? "—"}</td>
            <td>
              <span className={`state-tag ${ETAT_CLS[it.etat] ?? ""}`}>
                {it.etat}
              </span>
            </td>
            <td>{it.detail}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

export default App;
