import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

function App() {
  const [statusMsg, setStatusMsg] = useState("");
  const [isLoading, setIsLoading] = useState(false);

  async function handleBackup() {
    setIsLoading(true);
    setStatusMsg("Création du point de restauration en cours...");
    try {
      const response = await invoke<string>("create_restore_point");
      setStatusMsg(`Succès : ${response}`);
    } catch (error) {
      setStatusMsg(`Erreur : ${error}`);
    } finally {
      setIsLoading(false);
    }
  }

  async function handleOptimize() {
    setIsLoading(true);
    setStatusMsg("Optimisation des performances en cours...");
    try {
      const response = await invoke<string>("optimize_performance");
      setStatusMsg(`Succès : ${response}`);
    } catch (error) {
      setStatusMsg(`Erreur : ${error}`);
    } finally {
      setIsLoading(false);
    }
  }

  return (
    <main className="container">
      <h1>Windows 11 Optimizer</h1>
      <p>Optimisez votre système de manière modulaire et sécurisée.</p>

      <div className="card">
        <h2>1. Sécurité</h2>
        <p>Toujours créer un point de restauration avant d'effectuer des modifications.</p>
        <button onClick={handleBackup} disabled={isLoading}>
          Créer un point de restauration
        </button>
      </div>

      <div className="card">
        <h2>2. Performances</h2>
        <p>Ajuster les paramètres système pour maximiser les performances (ex: mode d'alimentation).</p>
        <button onClick={handleOptimize} disabled={isLoading}>
          Optimiser les performances
        </button>
      </div>

      {statusMsg && (
        <div className={`status-box ${statusMsg.startsWith("Erreur") ? "error" : "success"}`}>
          <p>{statusMsg}</p>
        </div>
      )}
    </main>
  );
}

export default App;
