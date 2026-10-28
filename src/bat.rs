//! Gera arquivos .bat equivalentes ao perfil escolhido (temporarios).

use crate::catalogo::*;
use std::path::{Path, PathBuf};

const ELEVAR: &str = r#"fltmc >nul 2>&1 || (echo Solicitando permissao de administrador... & powershell -NoProfile -Command "Start-Process -FilePath '%~f0' -Verb RunAs" & exit /b)"#;

fn cabecalho(titulo: &str) -> Vec<String> {
    vec![
        "@echo off".into(),
        "setlocal EnableExtensions".into(),
        "chcp 65001 >nul".into(),
        format!("title WinOpti - {titulo}"),
        ELEVAR.into(),
        r#"set "WO=%LOCALAPPDATA%\WinOpti""#.into(),
        r#"if not exist "%WO%" mkdir "%WO%""#.into(),
        "echo.".into(),
    ]
}

pub fn gerar(pasta: &Path, perfil: &str, sel: &[bool]) -> std::io::Result<(PathBuf, PathBuf)> {
    let escolhidos: Vec<&Item> = ITENS.iter().zip(sel.iter()).filter(|(_, s)| **s).map(|(i, _)| i).collect();

    let mut l = cabecalho(&format!("Perfil {perfil}"));
    l.push(format!("echo  ===== WinOpti - Perfil {perfil} ====="));
    l.push("echo  Alteracoes TEMPORARIAS: ao reiniciar o PC tudo volta ao normal.".into());
    l.push("echo  Para voltar sem reiniciar, execute WinOpti-Restaurar.bat".into());
    l.push("echo.".into());
    // limpa o registro do .bat no proximo login (o PC reiniciado ja esta normal)
    l.push(r#"reg add "HKCU\Software\Microsoft\Windows\CurrentVersion\RunOnce" /v WinOptiLimpar /d "cmd /c del /q \"%WO%\bat_*.txt\"" /f >nul"#.into());

    let svcs: Vec<&str> = escolhidos.iter().flat_map(|i| i.servicos.iter().copied()).collect();
    if !svcs.is_empty() {
        l.push("echo [Servicos - apenas parados, nada e desativado]".into());
        for s in svcs {
            l.push(format!(r#"call :parar "{s}""#));
        }
        l.push("echo.".into());
    }

    let procs: Vec<String> = escolhidos
        .iter()
        .flat_map(|i| i.processos.iter())
        .filter(|p| !p.starts_with("path:"))
        .map(|p| format!("{p}.exe").replace("*.exe", "*"))
        .collect();
    if !procs.is_empty() {
        l.push("echo [Aplicativos em segundo plano]".into());
        for p in procs {
            l.push(format!(r#"call :fechar "{p}""#));
        }
        l.push("timeout /t 4 /nobreak >nul".into());
        l.push("set SILENCIO=1".into());
        for p in escolhidos.iter().flat_map(|i| i.processos.iter()).filter(|p| !p.starts_with("path:")) {
            l.push(format!(r#"call :fechar "{}""#, format!("{p}.exe").replace("*.exe", "*")));
        }
        l.push("set SILENCIO=".into());
        l.push("echo.".into());
    }

    if escolhidos.iter().any(|i| i.energia) {
        l.push("echo [Energia]".into());
        l.push(r#"for /f "tokens=2 delims=:(" %%G in ('powercfg /getactivescheme') do set "ORIG=%%G""#.into());
        l.push(r#"set "ORIG=%ORIG: =%""#.into());
        l.push(format!(r#"if /i "%ORIG%"=="{GUID}" goto energia_ok"#, GUID = crate::sistema::GUID_ALTO_DESEMPENHO));
        l.push(r#"if not exist "%WO%\bat_energia.txt" (>"%WO%\bat_energia.txt" echo %ORIG%)"#.into());
        let g = crate::sistema::GUID_ALTO_DESEMPENHO;
        l.push(format!(r#"powercfg /setactive {g} >nul 2>&1 && goto energia_ativada"#));
        // plano escondido (Modern Standby): usa/cria uma copia
        l.push(r#"set "ALTO=""#.into());
        l.push(r#"if exist "%WO%\plano_alto.txt" set /p ALTO=<"%WO%\plano_alto.txt""#.into());
        l.push(r#"if defined ALTO powercfg /setactive %ALTO% >nul 2>&1 && goto energia_ativada"#.into());
        l.push(format!(r#"for /f "tokens=2 delims=:(" %%G in ('powercfg -duplicatescheme {g}') do set "ALTO=%%G""#));
        l.push(r#"if not defined ALTO goto energia_falhou"#.into());
        l.push(r#"set "ALTO=%ALTO: =%""#.into());
        l.push(r#">"%WO%\plano_alto.txt" echo %ALTO%"#.into());
        l.push(r#"powercfg /setactive %ALTO% >nul 2>&1 || goto energia_falhou"#.into());
        l.push(":energia_ativada".into());
        l.push(r#"set /p ORIG=<"%WO%\bat_energia.txt""#.into());
        l.push(r#"reg add "HKCU\Software\Microsoft\Windows\CurrentVersion\RunOnce" /v WinOptiEnergia /d "powercfg /setactive %ORIG%" /f >nul"#.into());
        l.push(":energia_ok".into());
        l.push("echo   [OK] Plano Alto desempenho ativo (o original volta no proximo login)".into());
        l.push("goto energia_fim".into());
        l.push(":energia_falhou".into());
        l.push("echo   [--] Plano Alto desempenho nao existe neste PC - use o modo Desempenho do fabricante".into());
        l.push(":energia_fim".into());
        l.push("echo.".into());
    }

    l.push(format!("echo  Perfil {perfil} aplicado. Bom jogo / bom trabalho!"));
    l.push("echo.".into());
    l.push("pause".into());
    l.push("exit /b".into());
    l.push("".into());
    l.push(":parar".into());
    l.push(r#"net stop "%~1" /y >nul 2>&1 && ((>>"%WO%\bat_servicos.txt" echo %~1) & echo   [OK] Servico parado: %~1)"#.into());
    l.push("exit /b".into());
    l.push(":fechar".into());
    l.push(r#"taskkill /f /t /im "%~1" >nul 2>&1 && if not defined SILENCIO echo   [OK] Fechado: %~1"#.into());
    l.push("exit /b".into());

    // ----------------------------------------------------- restaurar
    let mut r = cabecalho("Restaurar");
    r.push("echo  ===== WinOpti - Restaurando o estado normal =====".into());
    r.push("echo.".into());
    r.push(r#"if not exist "%WO%\bat_servicos.txt" goto sem_servicos"#.into());
    r.push(r#"for /f "usebackq delims=" %%S in ("%WO%\bat_servicos.txt") do call :iniciar "%%S""#.into());
    r.push(r#"del /q "%WO%\bat_servicos.txt" >nul 2>&1"#.into());
    r.push(":sem_servicos".into());
    r.push(r#"if not exist "%WO%\bat_energia.txt" goto sem_energia"#.into());
    r.push(r#"set /p ORIG=<"%WO%\bat_energia.txt""#.into());
    r.push(r#"powercfg /setactive %ORIG% >nul 2>&1 && echo   [OK] Plano de energia original restaurado"#.into());
    r.push(r#"del /q "%WO%\bat_energia.txt" >nul 2>&1"#.into());
    r.push(":sem_energia".into());
    r.push(r#"reg delete "HKCU\Software\Microsoft\Windows\CurrentVersion\RunOnce" /v WinOptiEnergia /f >nul 2>&1"#.into());
    r.push(r#"reg delete "HKCU\Software\Microsoft\Windows\CurrentVersion\RunOnce" /v WinOptiLimpar /f >nul 2>&1"#.into());
    r.push("echo.".into());
    r.push("echo  Servicos de volta. Os aplicativos fechados voltam quando voce abri-los".into());
    r.push("echo  ou ao reiniciar o computador.".into());
    r.push("echo.".into());
    r.push("choice /c SN /m \"Deseja reiniciar o computador agora\"".into());
    r.push("if errorlevel 2 exit /b".into());
    r.push(r#"shutdown /r /t 5 /c "WinOpti: reiniciando para voltar ao normal""#.into());
    r.push("exit /b".into());
    r.push("".into());
    r.push(":iniciar".into());
    r.push(r#"net start "%~1" >nul 2>&1 && echo   [OK] Servico iniciado: %~1"#.into());
    r.push("exit /b".into());

    let a = pasta.join(format!("WinOpti-{perfil}.bat"));
    let b = pasta.join("WinOpti-Restaurar.bat");
    std::fs::write(&a, l.join("\r\n") + "\r\n")?;
    std::fs::write(&b, r.join("\r\n") + "\r\n")?;
    Ok((a, b))
}

pub fn area_de_trabalho() -> PathBuf {
    let mut c = vec![];
    if let Ok(o) = std::env::var("OneDrive") {
        c.push(PathBuf::from(&o).join("Desktop"));
        c.push(PathBuf::from(&o).join("Área de Trabalho"));
    }
    if let Ok(u) = std::env::var("USERPROFILE") {
        c.push(PathBuf::from(&u).join("Desktop"));
        c.push(PathBuf::from(u));
    }
    c.into_iter().find(|p| p.is_dir()).unwrap_or_else(std::env::temp_dir)
}
