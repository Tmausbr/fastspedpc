//! Operacoes no sistema: servicos, processos, plano de energia, estado salvo.
//! Nada aqui altera o tipo de inicializacao de servicos: tudo e temporario.

use crate::catalogo::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use sysinfo::{Process, ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System, UpdateKind};

pub const GUID_ALTO_DESEMPENHO: &str = "8c5e7fda-e8bf-4a96-9a85-f6e4f6a3f1e5";
const RUNONCE: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\RunOnce";

// ============================================================ estado compartilhado
#[derive(Default)]
pub struct Shared {
    pub log: Mutex<Vec<String>>,
    pub ocupado: AtomicBool,
    pub varrendo: AtomicBool,
    pub varredura: Mutex<Option<Varredura>>,
    pub mudou: AtomicBool,
}

impl Shared {
    pub fn log(&self, s: impl Into<String>) {
        let mut l = self.log.lock().unwrap();
        l.push(s.into());
        if l.len() > 400 {
            l.drain(0..100);
        }
    }
}

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct Estado {
    pub perfil: String,
    pub boot: u64,
    pub inicio: u64,
    pub servicos: Vec<String>,
    pub apps: Vec<String>,
    pub energia_original: Option<String>,
    pub itens: Vec<String>,
}

pub fn pasta_dados() -> PathBuf {
    let base = std::env::var("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(|_| std::env::temp_dir());
    let p = base.join("WinOpti");
    let _ = std::fs::create_dir_all(&p);
    p
}

fn arq_estado() -> PathBuf {
    pasta_dados().join("estado.json")
}

pub fn agora() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Le o estado salvo. Se o PC foi reiniciado desde entao, o estado e descartado
/// (o Windows ja voltou ao normal sozinho) e retorna (None, true).
pub fn carregar_estado() -> (Option<Estado>, bool) {
    let txt = match std::fs::read_to_string(arq_estado()) {
        Ok(t) => t,
        Err(_) => return (None, false),
    };
    let est: Estado = match serde_json::from_str(&txt) {
        Ok(e) => e,
        Err(_) => return (None, false),
    };
    let boot = System::boot_time();
    if est.boot.abs_diff(boot) > 120 {
        let _ = std::fs::remove_file(arq_estado());
        return (None, true);
    }
    (Some(est), false)
}

fn salvar_estado(e: &Estado) {
    if let Ok(t) = serde_json::to_string_pretty(e) {
        let _ = std::fs::write(arq_estado(), t);
    }
}

// ============================================================ comandos
pub fn cmd(prog: &str, args: &[&str]) -> (bool, String) {
    let mut c = Command::new(prog);
    c.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    match c.output() {
        Ok(o) => (o.status.success(), String::from_utf8_lossy(&o.stdout).into_owned()),
        Err(e) => (false, e.to_string()),
    }
}

pub fn abrir(alvo: &str) {
    let _ = Command::new("explorer.exe").arg(alvo).spawn();
}

// ============================================================ servicos
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SvcEstado {
    NaoExiste,
    Parado,
    Rodando,
    Outro,
}

#[cfg(windows)]
pub fn svc_estado(nome: &str) -> SvcEstado {
    use windows_sys::Win32::System::Services::*;
    unsafe {
        let scm = OpenSCManagerW(std::ptr::null(), std::ptr::null(), SC_MANAGER_CONNECT);
        if scm.is_null() {
            return SvcEstado::Outro;
        }
        let w: Vec<u16> = nome.encode_utf16().chain(std::iter::once(0)).collect();
        let h = OpenServiceW(scm, w.as_ptr(), SERVICE_QUERY_STATUS);
        if h.is_null() {
            CloseServiceHandle(scm);
            return SvcEstado::NaoExiste;
        }
        let mut st: SERVICE_STATUS = std::mem::zeroed();
        let ok = QueryServiceStatus(h, &mut st);
        CloseServiceHandle(h);
        CloseServiceHandle(scm);
        if ok == 0 {
            return SvcEstado::Outro;
        }
        match st.dwCurrentState {
            SERVICE_RUNNING => SvcEstado::Rodando,
            SERVICE_STOPPED => SvcEstado::Parado,
            _ => SvcEstado::Outro,
        }
    }
}

#[cfg(not(windows))]
pub fn svc_estado(_nome: &str) -> SvcEstado {
    SvcEstado::NaoExiste
}

/// Para o servico (sem mudar o tipo de inicializacao). Retorna true se parou.
fn parar_servico(nome: &str) -> bool {
    let _ = cmd("net", &["stop", nome, "/y"]);
    for _ in 0..20 {
        match svc_estado(nome) {
            SvcEstado::Parado | SvcEstado::NaoExiste => return true,
            SvcEstado::Rodando => {}
            SvcEstado::Outro => {}
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    false
}

fn iniciar_servico(nome: &str) -> bool {
    if svc_estado(nome) == SvcEstado::Rodando {
        return true;
    }
    cmd("net", &["start", nome]).0 || svc_estado(nome) == SvcEstado::Rodando
}

// ============================================================ processos
pub fn stem(p: &Process) -> String {
    let n = p.name().to_string_lossy().to_lowercase();
    n.strip_suffix(".exe").map(|s| s.to_string()).unwrap_or(n)
}

pub fn caminho(p: &Process) -> String {
    p.exe().map(|e| e.to_string_lossy().into_owned()).unwrap_or_default()
}

pub fn casa(p: &Process, padroes: &[&str]) -> bool {
    let n = stem(p);
    let cam = caminho(p).to_lowercase();
    padroes.iter().any(|pad| {
        if let Some(t) = pad.strip_prefix("path:") {
            !cam.is_empty() && cam.contains(t)
        } else if let Some(pre) = pad.strip_suffix('*') {
            n.starts_with(pre)
        } else {
            n == *pad
        }
    })
}

pub fn protegido(p: &Process) -> bool {
    let n = stem(p);
    p.pid().as_u32() == std::process::id() || p.pid().as_u32() <= 4 || PROTEGIDOS.contains(&n.as_str())
}

pub fn sistema_processos() -> System {
    let mut s = System::new_with_specifics(RefreshKind::new());
    atualizar_processos(&mut s);
    s
}

pub fn atualizar_processos(s: &mut System) {
    s.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::new().with_cpu().with_memory().with_exe(UpdateKind::OnlyIfNotSet).with_cmd(UpdateKind::OnlyIfNotSet),
    );
}

fn ram_livre_gb() -> f64 {
    let mut s = System::new();
    s.refresh_memory();
    s.available_memory() as f64 / 1024f64.powi(3)
}

/// Converte o caminho de um app da Microsoft Store em "shell:AppsFolder\Familia!App".
fn alvo_reabrir(exe: &str) -> String {
    let low = exe.to_lowercase();
    if let Some(i) = low.find("\\windowsapps\\") {
        let resto = &exe[i + "\\windowsapps\\".len()..];
        if let Some(pasta) = resto.split('\\').next() {
            let partes: Vec<&str> = pasta.split('_').collect();
            if partes.len() >= 2 {
                return format!("shell:AppsFolder\\{}_{}!App", partes[0], partes[partes.len() - 1]);
            }
        }
    }
    exe.to_string()
}

// ============================================================ energia
fn achar_guid(s: &str) -> Option<String> {
    let b = s.as_bytes();
    for i in 0..b.len().saturating_sub(35) {
        let c = &s[i..i + 36];
        let ok = c.char_indices().all(|(j, ch)| {
            if [8, 13, 18, 23].contains(&j) { ch == '-' } else { ch.is_ascii_hexdigit() }
        });
        if ok {
            return Some(c.to_lowercase());
        }
    }
    None
}

/// Notebooks com "Modern Standby" escondem o plano Alto desempenho.
/// Cria uma copia dele uma unica vez (reaproveitada depois) e ativa.
fn ativar_copia_alto_desempenho() -> bool {
    let arq = pasta_dados().join("plano_alto.txt");
    if let Some(g) = std::fs::read_to_string(&arq).ok().and_then(|t| achar_guid(&t)) {
        if cmd("powercfg", &["/setactive", &g]).0 {
            return true;
        }
    }
    let (ok, saida) = cmd("powercfg", &["-duplicatescheme", GUID_ALTO_DESEMPENHO]);
    match (ok, achar_guid(&saida)) {
        (true, Some(g)) => {
            let _ = std::fs::write(&arq, &g);
            cmd("powercfg", &["/setactive", &g]).0
        }
        _ => false,
    }
}

pub fn plano_ativo() -> Option<String> {
    achar_guid(&cmd("powercfg", &["/getactivescheme"]).1)
}

// ============================================================ aplicar / restaurar
pub fn aplicar(sh: Arc<Shared>, sel: Vec<bool>, extras: Vec<String>, perfil: String) {
    sh.ocupado.store(true, Ordering::SeqCst);
    let ram_antes = ram_livre_gb();
    sh.log(format!("=== Aplicando perfil {perfil} (temporário) ==="));

    let (anterior, _) = carregar_estado();
    let mut est = anterior.unwrap_or_default();
    est.perfil = perfil.clone();
    est.boot = System::boot_time();
    if est.inicio == 0 {
        est.inicio = agora();
    }
    let mut servicos: BTreeSet<String> = est.servicos.iter().cloned().collect();
    let mut apps: BTreeSet<String> = est.apps.iter().cloned().collect();
    let mut itens: BTreeSet<String> = est.itens.iter().cloned().collect();

    let escolhidos: Vec<&Item> = ITENS.iter().zip(sel.iter()).filter(|(_, s)| **s).map(|(i, _)| i).collect();

    // ---- servicos
    for it in &escolhidos {
        for s in it.servicos {
            match svc_estado(s) {
                SvcEstado::Rodando => {
                    if parar_servico(s) {
                        servicos.insert(s.to_string());
                        itens.insert(it.id.to_string());
                        sh.log(format!("[OK] Serviço parado: {s}"));
                    } else {
                        sh.log(format!("[--] Não foi possível parar: {s} (protegido pelo Windows ou pelo fabricante)"));
                    }
                }
                _ => {}
            }
        }
    }

    // ---- processos (2 passadas: alguns agentes reabrem sozinhos)
    let mut sys = sistema_processos();
    for passada in 0..2 {
        if passada == 1 {
            std::thread::sleep(Duration::from_secs(4));
            atualizar_processos(&mut sys);
        }
        let mut fechados: BTreeSet<String> = BTreeSet::new();
        for p in sys.processes().values() {
            if protegido(p) {
                continue;
            }
            let n = stem(p);
            let item = escolhidos.iter().find(|it| !it.processos.is_empty() && casa(p, it.processos));
            let extra = extras.iter().any(|e| *e == n);
            if item.is_none() && !extra {
                continue;
            }
            let exe = caminho(p);
            if let Some(it) = item {
                itens.insert(it.id.to_string());
                if it.reabrir && !exe.is_empty() && passada == 0 {
                    apps.insert(alvo_reabrir(&exe));
                }
            } else if extra && !exe.is_empty() && passada == 0 {
                apps.insert(alvo_reabrir(&exe));
            }
            if p.kill() {
                fechados.insert(p.name().to_string_lossy().into_owned());
            }
        }
        for f in fechados {
            sh.log(format!("[OK] Fechado: {f}"));
        }
    }

    // ---- energia
    if escolhidos.iter().any(|i| i.energia) {
        let atual = plano_ativo();
        let copia = std::fs::read_to_string(pasta_dados().join("plano_alto.txt")).ok().and_then(|t| achar_guid(&t));
        if atual.as_deref() == Some(GUID_ALTO_DESEMPENHO) || (atual.is_some() && atual == copia) {
            sh.log("[OK] Plano Alto desempenho já está ativo");
        } else if cmd("powercfg", &["/setactive", GUID_ALTO_DESEMPENHO]).0 || ativar_copia_alto_desempenho() {
            if est.energia_original.is_none() {
                est.energia_original = atual.clone();
            }
            if let Some(orig) = &est.energia_original {
                let v = format!("powercfg /setactive {orig}");
                let _ = cmd("reg", &["add", RUNONCE, "/v", "WinOptiEnergia", "/d", &v, "/f"]);
            }
            itens.insert("energia".into());
            sh.log("[OK] Plano de energia: Alto desempenho (volta sozinho no próximo login)");
        } else {
            sh.log("[--] Plano Alto desempenho não existe neste PC. Use o modo Desempenho do app do fabricante.");
        }
    }

    est.servicos = servicos.into_iter().collect();
    est.apps = apps.into_iter().collect();
    est.itens = itens.into_iter().collect();
    salvar_estado(&est);

    std::thread::sleep(Duration::from_secs(1));
    let ram_depois = ram_livre_gb();
    sh.log(format!(
        "=== Perfil {perfil} ativo. RAM livre: {ram_antes:.1} GB -> {ram_depois:.1} GB. Reinicie ou clique em Restaurar para voltar ao normal ==="
    ));
    sh.mudou.store(true, Ordering::SeqCst);
    sh.ocupado.store(false, Ordering::SeqCst);
}

pub fn restaurar(sh: Arc<Shared>, reabrir_apps: bool) {
    sh.ocupado.store(true, Ordering::SeqCst);
    sh.log("=== Restaurando o estado normal ===");
    let (est, _) = carregar_estado();
    let mut servicos: Vec<String> = est.as_ref().map(|e| e.servicos.clone()).unwrap_or_default();
    let mut energia = est.as_ref().and_then(|e| e.energia_original.clone());

    // tambem desfaz o que foi feito pelos arquivos .bat
    let dir = pasta_dados();
    if let Ok(t) = std::fs::read_to_string(dir.join("bat_servicos.txt")) {
        servicos.extend(t.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()));
    }
    if energia.is_none() {
        if let Ok(t) = std::fs::read_to_string(dir.join("bat_energia.txt")) {
            energia = achar_guid(&t);
        }
    }
    servicos.sort();
    servicos.dedup();

    for s in &servicos {
        if iniciar_servico(s) {
            sh.log(format!("[OK] Serviço iniciado: {s}"));
        } else {
            sh.log(format!("[--] {s} não iniciou (pode ser de início sob demanda; o Windows inicia quando precisar)"));
        }
    }
    if let Some(g) = energia {
        if cmd("powercfg", &["/setactive", &g]).0 {
            sh.log("[OK] Plano de energia original restaurado");
        }
    }
    let _ = cmd("reg", &["delete", RUNONCE, "/v", "WinOptiEnergia", "/f"]);
    let _ = cmd("reg", &["delete", RUNONCE, "/v", "WinOptiLimpar", "/f"]);

    if reabrir_apps {
        if let Some(e) = &est {
            for a in &e.apps {
                abrir(a);
                let nome = Path::new(a).file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or(a.clone());
                sh.log(format!("[OK] Reabrindo: {nome}"));
                std::thread::sleep(Duration::from_millis(600));
            }
        }
    }
    let _ = std::fs::remove_file(arq_estado());
    let _ = std::fs::remove_file(dir.join("bat_servicos.txt"));
    let _ = std::fs::remove_file(dir.join("bat_energia.txt"));
    sh.log("=== Tudo de volta ao normal ===");
    sh.mudou.store(true, Ordering::SeqCst);
    sh.ocupado.store(false, Ordering::SeqCst);
}

pub fn limpar_temporarios(sh: Arc<Shared>) {
    sh.ocupado.store(true, Ordering::SeqCst);
    sh.log("=== Limpando arquivos temporários ===");
    let mut pastas = vec![std::env::temp_dir()];
    if let Ok(w) = std::env::var("SystemRoot") {
        pastas.push(PathBuf::from(w).join("Temp"));
    }
    let mut total: u64 = 0;
    let mut n = 0u32;
    for p in pastas {
        if let Ok(rd) = std::fs::read_dir(&p) {
            for e in rd.flatten() {
                let path = e.path();
                if path.file_name().map(|f| f.to_string_lossy().contains("WinOpti")).unwrap_or(false) {
                    continue;
                }
                let tam = tamanho(&path);
                let ok = if path.is_dir() { std::fs::remove_dir_all(&path).is_ok() } else { std::fs::remove_file(&path).is_ok() };
                if ok {
                    total += tam;
                    n += 1;
                }
            }
        }
    }
    sh.log(format!("[OK] {n} itens removidos, {:.0} MB liberados (arquivos em uso foram ignorados)", total as f64 / 1048576.0));
    sh.ocupado.store(false, Ordering::SeqCst);
}

fn tamanho(p: &Path) -> u64 {
    if p.is_dir() {
        std::fs::read_dir(p).map(|rd| rd.flatten().map(|e| tamanho(&e.path())).sum()).unwrap_or(0)
    } else {
        p.metadata().map(|m| m.len()).unwrap_or(0)
    }
}

// ============================================================ varredura
#[derive(Clone, Default)]
pub struct ItemVarredura {
    pub ativo: bool,
    pub detalhe: String,
    pub ram_mb: f64,
}

#[derive(Clone)]
pub struct Pesado {
    pub nome: String,
    pub stem: String,
    pub qtd: usize,
    pub ram_mb: f64,
    pub cpu: f32,
    pub caminho: String,
}

#[derive(Clone)]
pub struct Alerta {
    pub grave: bool,
    pub texto: String,
}

#[derive(Clone, Default)]
pub struct Varredura {
    pub itens: Vec<ItemVarredura>,
    pub pesados: Vec<Pesado>,
    pub alertas: Vec<Alerta>,
    pub total_proc: usize,
    pub economia_mb: f64,
}

pub fn varrer(sh: Arc<Shared>) {
    sh.varrendo.store(true, Ordering::SeqCst);
    let mut sys = sistema_processos();
    std::thread::sleep(Duration::from_millis(1500));
    atualizar_processos(&mut sys);
    let nucleos = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1) as f32;

    let mut v = Varredura { total_proc: sys.processes().len(), ..Default::default() };

    for it in ITENS {
        let mut iv = ItemVarredura::default();
        let mut partes = vec![];
        let mut rodando = 0;
        for s in it.servicos {
            if svc_estado(s) == SvcEstado::Rodando {
                rodando += 1;
            }
        }
        if rodando > 0 {
            partes.push(format!("{rodando} serviço(s) rodando"));
        }
        let mut qtd = 0;
        if !it.processos.is_empty() {
            for p in sys.processes().values() {
                if casa(p, it.processos) && !protegido(p) {
                    qtd += 1;
                    iv.ram_mb += p.memory() as f64 / 1048576.0;
                }
            }
            if qtd > 0 {
                partes.push(format!("{qtd} processo(s), {:.0} MB", iv.ram_mb));
            }
        }
        if it.energia {
            let a = plano_ativo();
            let copia = std::fs::read_to_string(pasta_dados().join("plano_alto.txt")).ok().and_then(|t| achar_guid(&t));
            if a.as_deref() == Some(GUID_ALTO_DESEMPENHO) || (a.is_some() && a == copia) {
                partes.push("já ativo".into());
            } else {
                iv.ativo = true;
                partes.push("plano atual não é Alto desempenho".into());
            }
        }
        iv.ativo = iv.ativo || rodando > 0 || qtd > 0;
        iv.detalhe = if partes.is_empty() { "não encontrado / já parado".into() } else { partes.join(" | ") };
        v.itens.push(iv);
    }

    // processos pesados fora do catalogo
    let mut mapa: std::collections::BTreeMap<String, Pesado> = Default::default();
    for p in sys.processes().values() {
        if protegido(p) || ITENS.iter().any(|i| !i.processos.is_empty() && casa(p, i.processos)) {
            continue;
        }
        let cam = caminho(p);
        let low = cam.to_lowercase();
        if cam.is_empty() || low.starts_with("c:\\windows\\") {
            continue;
        }
        let st = stem(p);
        let e = mapa.entry(st.clone()).or_insert(Pesado {
            nome: p.name().to_string_lossy().into_owned(),
            stem: st,
            qtd: 0,
            ram_mb: 0.0,
            cpu: 0.0,
            caminho: cam.clone(),
        });
        e.qtd += 1;
        e.ram_mb += p.memory() as f64 / 1048576.0;
        e.cpu += p.cpu_usage() / nucleos;
    }
    let mut pes: Vec<Pesado> = mapa.into_values().filter(|p| p.ram_mb >= 60.0 || p.cpu >= 1.0).collect();
    pes.sort_by(|a, b| (b.ram_mb + b.cpu as f64 * 50.0).partial_cmp(&(a.ram_mb + a.cpu as f64 * 50.0)).unwrap());
    pes.truncate(20);
    v.pesados = pes;

    // seguranca
    for p in sys.processes().values() {
        let n = stem(p);
        let cam = caminho(p);
        let low = cam.to_lowercase();
        let cmdl = p.cmd().iter().map(|c| c.to_string_lossy()).collect::<Vec<_>>().join(" ").to_lowercase();
        if MINERADORES.iter().any(|m| n.contains(m)) {
            v.alertas.push(Alerta { grave: true, texto: format!("Nome de minerador: {} (PID {}) {}", p.name().to_string_lossy(), p.pid(), cam) });
        }
        if CMD_MINERACAO.iter().any(|m| cmdl.contains(m)) {
            v.alertas.push(Alerta { grave: true, texto: format!("Linha de comando de mineração: {} (PID {}) {}", p.name().to_string_lossy(), p.pid(), cam) });
        }
        if NOMES_SISTEMA.contains(&n.as_str()) && !low.is_empty() && !low.starts_with("c:\\windows\\") {
            v.alertas.push(Alerta { grave: true, texto: format!("Imitando processo do Windows: {} rodando de {}", p.name().to_string_lossy(), cam) });
        }
        if (low.contains("\\appdata\\local\\temp\\") || low.contains("\\windows\\temp\\") || low.contains("\\users\\public\\") || low.contains("$recycle.bin"))
            && low.ends_with(".exe")
        {
            v.alertas.push(Alerta { grave: false, texto: format!("Executável rodando de pasta temporária: {cam} (instaladores são normais aqui)") });
        }
        let cpu = p.cpu_usage() / nucleos;
        if cpu > 25.0 && !protegido(p) {
            v.alertas.push(Alerta { grave: false, texto: format!("Uso alto de CPU agora: {} {:.0}% ({})", p.name().to_string_lossy(), cpu, cam) });
        }
    }
    // hosts
    if let Ok(w) = std::env::var("SystemRoot") {
        if let Ok(h) = std::fs::read_to_string(Path::new(&w).join("System32\\drivers\\etc\\hosts")) {
            let bloqueios: Vec<&str> = h
                .lines()
                .map(|l| l.trim())
                .filter(|l| !l.is_empty() && !l.starts_with('#'))
                .filter(|l| ["kaspersky", "microsoft", "windowsupdate", "eset", "avast", "malwarebytes", "virustotal"].iter().any(|k| l.to_lowercase().contains(k)))
                .collect();
            if !bloqueios.is_empty() {
                v.alertas.push(Alerta { grave: true, texto: format!("Arquivo hosts bloqueando sites de segurança: {}", bloqueios.join(" ; ")) });
            }
        }
    }

    v.economia_mb = ITENS.iter().zip(v.itens.iter()).filter(|(i, _)| i.gamer).map(|(_, s)| s.ram_mb).sum();
    *sh.varredura.lock().unwrap() = Some(v);
    sh.varrendo.store(false, Ordering::SeqCst);
}
