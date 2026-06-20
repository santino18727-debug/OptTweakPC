# AGENTS.md — OptTweakPC

Conventions GabiDevFamily pour tout agent de code async (Jules, Codex, etc.) travaillant
sur ce dépôt. Lis ce fichier **avant** d'ouvrir une PR. En cas de doute sur une décision
d'archi ou une action système risquée → **arrête-toi et décris-la dans la PR**, ne l'exécute pas.

## 1. Ce qu'est le projet

Optimiseur **Windows 11** en **Tauri 2** : backend **Rust** (`src-tauri/`) + frontend
**React 19 + TypeScript + Vite 7** (`src/`). UI et messages utilisateur **en français**
(localisation `fr-CH`). Embryonnaire : quelques commandes Tauri exposées, beaucoup reste à faire.

- Rust : edition 2021, crate `tauri_app_lib` (cf. `src-tauri/Cargo.toml`).
- Front : React 19, TS `~5.8`, Vite 7, `@tauri-apps/api` v2. Dev URL `http://localhost:1420`.
- Identifiant bundle : `com.gabidevfamily.opttweakpc`.

## 2. Structure du dépôt

```
src/                       Frontend React/TS
  App.tsx                  UI principale (cartes, invoke des commandes, journal)
  main.tsx, App.css, vite-env.d.ts
src-tauri/
  Cargo.toml               manifeste Rust
  tauri.conf.json          config app (fenêtre, bundle, identifier)
  capabilities/default.json  permissions Tauri
  src/
    lib.rs                 run() — enregistre invoke_handler![...] (point d'entrée des commandes)
    main.rs                bin mince qui appelle tauri_app_lib::run()
    core/    backup.rs (create_restore_point), system.rs (check_admin_rights, get_system_info)
    modules/ performance.rs (optimize_performance), startup.rs (audit/preview/clean/restore_startup)
vite.config.ts, tsconfig.json, package.json
```

Toute nouvelle commande Rust = fonction `#[tauri::command]` dans `core/` ou `modules/`,
PUIS ajoutée à la liste `generate_handler![...]` de `lib.rs`, PUIS appelée côté front via
`invoke<T>("nom_commande")`. Les trois étapes ensemble, sinon la commande est invisible.

## 3. Build & test

```bash
npm install
npm run dev          # front Vite seul (http://localhost:1420)
npm run tauri dev    # app complète (Rust + front) — nécessite la toolchain Rust + WebView2
npm run build        # tsc + vite build (vérifie le typage TS, bloque sur erreur de type)
npm run tauri build  # bundle release Windows

# côté Rust (depuis src-tauri/)
cargo build
cargo test
cargo clippy --all-targets -- -D warnings   # lint Rust, à viser zéro warning
cargo fmt --check
```

Pas encore de suite de tests front établie ; si tu ajoutes de la logique testable, ajoute
les tests Rust (`#[cfg(test)] mod tests`) en priorité (c'est là qu'est la logique système).

## 4. Conventions de code

### Rust
- Une commande = `pub fn` annotée `#[tauri::command]`, renvoyant `Result<T, String>` (l'erreur
  string remonte au front via le `.catch()`). Noms `snake_case` (ex. `clean_startup`).
- Regrouper par domaine : `core/` = primitives système (admin, restore point, info), `modules/`
  = features optimisation (performance, startup). Déclarer chaque sous-module dans le `mod.rs`.
- Pattern **dry-run obligatoire pour toute action qui modifie le système** : exposer un
  `preview_*` (lecture seule) AVANT le `clean_*`/`apply_*`, et un `restore_*` qui rejoue un
  snapshot. C'est le contrat UX existant (cf. startup) — toute nouvelle action destructive le suit.
- Pas de `.unwrap()`/`.expect()` sur des appels système faillibles dans le chemin runtime
  (sauf `run()` au boot). Propager l'erreur en `Result`.

### TypeScript / React
- Interfaces TS qui **reflètent exactement** les structs Rust sérialisées (champs en français
  comme dans le Rust : `programme`, `emplacement`, `etat`, `categorie` — cf. `App.tsx`). Ne pas
  angliciser un champ d'un côté seulement.
- `invoke<T>("commande")` typé. Toujours `try/catch` → état d'erreur affiché à l'utilisateur.
- Strings UI en français, format heure/date `fr-CH` (`toLocaleTimeString("fr-CH")`).
- Pas de `any`. Typage strict (le `npm run build` passe par `tsc`).

## 5. Naming & style

- Rust : `snake_case` (fonctions/champs), `CamelCase` (types), modules en fichiers `snake_case.rs`.
- TS : `camelCase` (vars/fonctions), `PascalCase` (composants/types/interfaces).
- Commentaires concis en français côté front (cf. style existant dans `App.tsx`).
- Toute action système porte un **niveau de risque** visible à l'utilisateur (`safe`/`read`/
  `moderate` — cf. `RiskBadge`). Une nouvelle action classée honnêtement, pas minimisée.

## 6. Commits & PR

- Messages courts en français, préfixes : `fix:`, `feat:`, `refactor:`, `docs:`, `chore:`.
- **Un diff = un sujet.** Pas de reformat de fichier entier mélangé à une feature.
- Ne pas lancer un formateur global qui réécrit des lignes non touchées → édition ciblée,
  `cargo fmt`/`prettier` uniquement sur les fichiers que tu modifies réellement.
- Pas de `git push` direct sur la branche principale : tu ouvres une PR, Eric review et merge.

## 7. Garde-fous — NE PAS toucher / NE PAS faire

- ❌ **Pas de `git push` ni de release/bundle publié.** PR uniquement, merge par Eric.
- ⚠️ **Code qui touche le système Windows réel** (registre, services, plan d'alim, démarrage,
  points de restauration) : c'est le cœur sensible de l'app. Toute nouvelle commande qui écrit
  dans le système DOIT (a) avoir un `preview_*` dry-run, (b) être réversible (`restore_*`),
  (c) être classée `moderate`/`risk` dans l'UI, (d) vérifier les droits admin (`check_admin_rights`).
- ❌ Ne **jamais** exécuter une commande système destructive depuis l'agent pour "tester" sur la
  machine de dev — la logique se valide par `cargo test` sur de la donnée mockée, pas en touchant
  le vrai registre/les vrais services.
- ❌ Pas de secrets en dur. Pas d'appel réseau/télémétrie ajouté sans demande explicite (app
  locale, offline-first).
- ❌ Pas de nouvelle dépendance npm/cargo sans justification dans la PR. Vérifie qu'un package
  existe et est déjà déclaré avant de l'importer (pas d'hallucination de crate/version).
- ⚠️ Élargir `capabilities/default.json` (permissions Tauri) = décision de sécurité → **décris-la
  dans la PR**, ne l'élargis pas silencieusement.
- ⚠️ Le débridage UAC / l'élévation de privilèges est un sujet sensible identifié : si ta tâche
  l'implique, expose l'approche dans la PR avant d'implémenter.
