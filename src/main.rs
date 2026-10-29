#![cfg_attr(windows, windows_subsystem = "windows")]

mod bat;
mod catalogo;
mod conhecidos;
mod diagnostico;
mod sistema;
mod tela_diag;

use tela_diag::cor_nota;

use catalogo::*;
use diagnostico::{Acao, Categoria, Diagnostico, Gravidade};
use eframe::egui::{self, Align, Align2, Color32, FontId, Layout, Pos2, RichText, Rounding, Sense, Shape, Stroke, Vec2};
use sistema::*;
use std::collections::{BTreeSet, HashSet};
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};
use sysinfo::{Disks, System};

// ------------------------------------------------------------------ cores
const FUNDO: Color32 = Color32::from_rgb(244, 247, 251);
const CARTAO: Color32 = Color32::WHITE;
const BORDA: Color32 = Color32::from_rgb(226, 232, 240);
const AZUL: Color32 = Color32::from_rgb(21, 101, 216);
const AZUL_CLARO: Color32 = Color32::from_rgb(232, 241, 253);
const VERDE: Color32 = Color32::from_rgb(34, 150, 100);
const VERDE_CLARO: Color32 = Color32::from_rgb(232, 247, 239);
const AMARELO: Color32 = Color32::from_rgb(214, 148, 20);
const AMARELO_CLARO: Color32 = Color32::from_rgb(253, 246, 228);
const VERMELHO: Color32 = Color32::from_rgb(208, 60, 60);
const VERMELHO_CLARO: Color32 = Color32::from_rgb(253, 236, 236);
const TEXTO: Color32 = Color32::from_rgb(24, 33, 48);
const SUAVE: Color32 = Color32::from_rgb(96, 106, 122);
const TRILHO: Color32 = Color32::from_rgb(229, 233, 239);

#[derive(PartialEq, Clone, Copy)]
enum Tela {
    Inicio,
    Otimizacao,
    Diagnostico,
    Varredura,
    Scripts,
    Atividade,
    Sobre,
}

enum Confirmar {
    Nada,
    Reiniciar,
    Aplicar,
}

struct App {
    tela: Tela,
    perfil: Perfil,
    sel: Vec<bool>,
    sys: System,
    discos: Disks,
    ult: Instant,
    ult_disco: Instant,
    cpu: f32,
    ram: f32,
    ram_txt: String,
    disco: f32,
    disco_txt: String,
    sh: Arc<Shared>,
    estado: Option<Estado>,
    reiniciado: bool,
    confirmar: Confirmar,
    extras: BTreeSet<String>,
    pasta_bat: PathBuf,
    msg_bat: Option<(bool, String)>,
    reabrir: bool,
    extras_serv: BTreeSet<String>,
    filtro_cat: Option<Categoria>,
    mostrar_info: bool,
    aba_diag: u8,
    feitos: HashSet<String>,
    msg_diag: Option<String>,
}

impl App {
    fn new(cc: &eframe::CreationContext) -> Self {
        let ctx = &cc.egui_ctx;
        let mut v = egui::Visuals::light();
        v.panel_fill = FUNDO;
        v.window_fill = CARTAO;
        v.selection.bg_fill = AZUL;
        v.widgets.inactive.rounding = Rounding::same(8.0);
        v.widgets.hovered.rounding = Rounding::same(8.0);
        v.widgets.active.rounding = Rounding::same(8.0);
        v.widgets.noninteractive.fg_stroke.color = TEXTO;
        ctx.set_visuals(v);
        ctx.style_mut(|s| {
            s.spacing.item_spacing = Vec2::new(8.0, 8.0);
            s.spacing.button_padding = Vec2::new(14.0, 8.0);
            s.text_styles.insert(egui::TextStyle::Body, FontId::proportional(15.0));
            s.text_styles.insert(egui::TextStyle::Button, FontId::proportional(15.0));
            s.text_styles.insert(egui::TextStyle::Small, FontId::proportional(12.5));
        });

        let (estado, reiniciado) = carregar_estado();
        let perfil = estado
            .as_ref()
            .map(|e| match e.perfil.as_str() {
                "Gamer" => Perfil::Gamer,
                "Otimizado" => Perfil::Otimizado,
                "Personalizado" => Perfil::Personalizado,
                _ => Perfil::Otimizado,
            })
            .unwrap_or(Perfil::Otimizado);
        let sel = perfil.selecao().unwrap_or_else(|| {
            let ids: Vec<String> = estado.as_ref().map(|e| e.itens.clone()).unwrap_or_default();
            ITENS.iter().map(|i| ids.iter().any(|x| x == i.id)).collect()
        });

        let sh = Arc::new(Shared::default());
        if reiniciado {
            sh.log("O computador foi reiniciado: o Windows voltou ao estado normal automaticamente.");
        }
        let sh2 = sh.clone();
        std::thread::spawn(move || varrer(sh2));
        let sh3 = sh.clone();
        std::thread::spawn(move || diagnostico::executar(sh3));

        let mut sys = System::new();
        sys.refresh_cpu_usage();
        sys.refresh_memory();
        App {
            tela: Tela::Inicio,
            perfil,
            sel,
            sys,
            discos: Disks::new_with_refreshed_list(),
            ult: Instant::now() - Duration::from_secs(5),
            ult_disco: Instant::now(),
            cpu: 0.0,
            ram: 0.0,
            ram_txt: String::new(),
            disco: 0.0,
            disco_txt: String::new(),
            sh,
            estado,
            reiniciado,
            confirmar: Confirmar::Nada,
            extras: BTreeSet::new(),
            pasta_bat: bat::area_de_trabalho(),
            msg_bat: None,
            reabrir: true,
            extras_serv: BTreeSet::new(),
            filtro_cat: None,
            mostrar_info: false,
            aba_diag: 0,
            feitos: HashSet::new(),
            msg_diag: None,
        }
    }

    fn atualizar_metricas(&mut self) {
        if self.ult.elapsed() < Duration::from_millis(1500) {
            return;
        }
        self.ult = Instant::now();
        self.sys.refresh_cpu_usage();
        self.sys.refresh_memory();
        self.cpu = self.sys.global_cpu_usage();
        let tot = self.sys.total_memory() as f32;
        let usada = tot - self.sys.available_memory() as f32;
        self.ram = if tot > 0.0 { usada / tot * 100.0 } else { 0.0 };
        self.ram_txt = format!("{:.1} de {:.1} GB", usada / 1073741824.0, tot / 1073741824.0);
        if self.ult_disco.elapsed() > Duration::from_secs(20) || self.disco_txt.is_empty() {
            self.ult_disco = Instant::now();
            self.discos.refresh_list();
            let d = self
                .discos
                .list()
                .iter()
                .find(|d| d.mount_point().to_string_lossy().to_uppercase().starts_with("C:"))
                .or_else(|| self.discos.list().first());
            if let Some(d) = d {
                let t = d.total_space() as f32;
                let l = d.available_space() as f32;
                self.disco = if t > 0.0 { (t - l) / t * 100.0 } else { 0.0 };
                self.disco_txt = format!("{:.0} GB livres", l / 1073741824.0);
            }
        }
        if self.sh.mudou.swap(false, Ordering::SeqCst) {
            self.estado = carregar_estado().0;
            self.reiniciado = false;
            let sh = self.sh.clone();
            std::thread::spawn(move || varrer(sh));
        }
    }

    fn ocupado(&self) -> bool {
        self.sh.ocupado.load(Ordering::SeqCst)
    }

    fn escolher_perfil(&mut self, p: Perfil) {
        self.perfil = p;
        if let Some(s) = p.selecao() {
            self.sel = s;
        }
    }

    fn aplicar(&mut self) {
        if self.ocupado() {
            return;
        }
        if self.perfil == Perfil::Padrao {
            self.restaurar();
            return;
        }
        let sh = self.sh.clone();
        let sel = self.sel.clone();
        let extras: Vec<String> = self.extras.iter().cloned().collect();
        let extras_serv: Vec<String> = self.extras_serv.iter().cloned().collect();
        let nome = self.perfil.nome().to_string();
        self.tela = Tela::Atividade;
        std::thread::spawn(move || aplicar(sh, sel, extras, extras_serv, nome));
    }

    fn restaurar(&mut self) {
        if self.ocupado() {
            return;
        }
        let sh = self.sh.clone();
        let r = self.reabrir;
        self.tela = Tela::Atividade;
        std::thread::spawn(move || restaurar(sh, r));
    }
}

// ------------------------------------------------------------------ widgets
fn cartao<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> egui::InnerResponse<R> {
    egui::Frame::none()
        .fill(CARTAO)
        .stroke(Stroke::new(1.0_f32, BORDA))
        .rounding(Rounding::same(12.0))
        .inner_margin(egui::Margin::same(18.0))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
}

fn titulo(ui: &mut egui::Ui, icone: &str, t: &str, sub: &str) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(icone).size(20.0).color(AZUL));
        ui.label(RichText::new(t).size(19.0).strong().color(TEXTO));
    });
    if !sub.is_empty() {
        ui.label(RichText::new(sub).size(13.5).color(SUAVE));
    }
    ui.add_space(6.0);
}

fn arco(c: Pos2, r: f32, a0: f32, a1: f32) -> Vec<Pos2> {
    let n = 72;
    (0..=n)
        .map(|i| {
            let a = a0 + (a1 - a0) * i as f32 / n as f32;
            Pos2::new(c.x + a.cos() * r, c.y + a.sin() * r)
        })
        .collect()
}

fn medidor(ui: &mut egui::Ui, rotulo: &str, pct: f32, sub: &str, invertido: bool) {
    let (cor, palavra) = if invertido {
        if pct >= 70.0 { (VERDE, "Bom") } else if pct >= 45.0 { (AMARELO, "Regular") } else { (VERMELHO, "Baixo") }
    } else if pct < 50.0 {
        (VERDE, "Baixo")
    } else if pct < 80.0 {
        (AMARELO, "Normal")
    } else {
        (VERMELHO, "Alto")
    };
    ui.vertical_centered(|ui| {
        ui.label(RichText::new(rotulo).size(13.5).color(SUAVE));
        let (rect, _) = ui.allocate_exact_size(Vec2::new(110.0, 100.0), Sense::hover());
        let c = rect.center();
        let p = ui.painter();
        let r = 40.0;
        p.add(Shape::line(arco(c, r, 0.0, std::f32::consts::TAU), Stroke::new(9.0_f32, TRILHO)));
        let a0 = -std::f32::consts::FRAC_PI_2;
        let frac = (pct / 100.0).clamp(0.0, 1.0).max(0.01);
        p.add(Shape::line(arco(c, r, a0, a0 + std::f32::consts::TAU * frac), Stroke::new(9.0_f32, cor)));
        p.text(c, Align2::CENTER_CENTER, format!("{:.0}%", pct), FontId::proportional(21.0), TEXTO);
        ui.label(RichText::new(palavra).size(15.0).strong().color(cor));
        ui.label(RichText::new(sub).size(12.0).color(SUAVE));
    });
}

fn botao(ui: &mut egui::Ui, texto: &str, sub: &str, cor: Color32, largura: f32, habilitado: bool) -> bool {
    let primario = cor != CARTAO;
    let fg = if primario { Color32::WHITE } else { TEXTO };
    let altura = if sub.is_empty() { 42.0 } else { 56.0 };
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(largura, altura), if habilitado { Sense::click() } else { Sense::hover() });
    let hover = habilitado && resp.hovered();
    let mut fill = if hover {
        if primario { Color32::from_rgb(14, 84, 186) } else { Color32::from_rgb(240, 245, 252) }
    } else {
        cor
    };
    if !habilitado {
        fill = if primario { Color32::from_rgb(150, 182, 230) } else { Color32::from_rgb(248, 249, 251) };
    }
    let p = ui.painter();
    p.rect(rect, Rounding::same(9.0), fill, Stroke::new(1.0_f32, if primario { fill } else if hover { AZUL } else { BORDA }));
    let g1 = p.layout_no_wrap(texto.to_string(), FontId::proportional(16.0), if habilitado { fg } else { SUAVE });
    let g2 = p.layout_no_wrap(
        sub.to_string(),
        FontId::proportional(12.0),
        if primario { Color32::from_rgb(220, 232, 252) } else { SUAVE },
    );
    let total = g1.size().y + if sub.is_empty() { 0.0 } else { g2.size().y + 2.0 };
    let y0 = rect.center().y - total / 2.0;
    p.galley(Pos2::new(rect.center().x - g1.size().x / 2.0, y0), g1.clone(), fg);
    if !sub.is_empty() {
        p.galley(Pos2::new(rect.center().x - g2.size().x / 2.0, y0 + g1.size().y + 2.0), g2, SUAVE);
    }
    if hover {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    habilitado && resp.clicked()
}

fn chip(ui: &mut egui::Ui, txt: &str, fg: Color32, bg: Color32) {
    egui::Frame::none().fill(bg).rounding(Rounding::same(10.0)).inner_margin(egui::Margin::symmetric(8.0, 2.0)).show(ui, |ui| {
        ui.label(RichText::new(txt).size(12.0).color(fg));
    });
}

// ------------------------------------------------------------------ app
impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.atualizar_metricas();
        ctx.request_repaint_after(Duration::from_millis(500));

        self.barra_lateral(ctx);
        self.painel_acoes(ctx);
        egui::CentralPanel::default().frame(egui::Frame::none().fill(FUNDO).inner_margin(egui::Margin::same(22.0))).show(ctx, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| match self.tela {
                Tela::Inicio => self.tela_inicio(ui),
                Tela::Otimizacao => self.tela_otimizacao(ui),
                Tela::Diagnostico => self.tela_diagnostico(ui),
                Tela::Varredura => self.tela_varredura(ui),
                Tela::Scripts => self.tela_scripts(ui),
                Tela::Atividade => self.tela_atividade(ui),
                Tela::Sobre => self.tela_sobre(ui),
            });
        });
        self.dialogos(ctx);
    }
}

impl App {
    fn barra_lateral(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("nav")
            .exact_width(236.0)
            .resizable(false)
            .frame(egui::Frame::none().fill(CARTAO).stroke(Stroke::new(1.0_f32, BORDA)).inner_margin(egui::Margin::same(16.0)))
            .show(ctx, |ui| {
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(46.0), Sense::hover());
                    ui.painter().rect_filled(r, Rounding::same(11.0), AZUL);
                    let c = r.center() + Vec2::new(0.0, 4.0);
                    ui.painter().add(Shape::line(arco(c, 14.0, std::f32::consts::PI, std::f32::consts::TAU), Stroke::new(3.0_f32, Color32::WHITE)));
                    ui.painter().line_segment([c, c + Vec2::new(8.0, -9.0)], Stroke::new(3.0_f32, Color32::WHITE));
                    ui.vertical(|ui| {
                        ui.label(RichText::new("WinOpti").size(23.0).strong().color(TEXTO));
                        ui.label(RichText::new("Mais desempenho, sem\nmudar nada para sempre").size(11.5).color(SUAVE));
                    });
                });
                ui.add_space(20.0);
                let itens = [
                    (Tela::Inicio, "🏠  Início"),
                    (Tela::Otimizacao, "⚙  Serviços e Aplicativos"),
                    (Tela::Diagnostico, "🔎  Diagnóstico (mapa)"),
                    (Tela::Varredura, "⚡  Varredura rápida"),
                    (Tela::Scripts, "📄  Gerar Script (.bat)"),
                    (Tela::Atividade, "📋  Registro de Atividade"),
                    (Tela::Sobre, "ℹ  Como funciona"),
                ];
                for (t, txt) in itens {
                    let ativo = self.tela == t;
                    let b = egui::Button::new(RichText::new(txt).size(15.0).color(if ativo { AZUL } else { TEXTO }))
                        .fill(if ativo { AZUL_CLARO } else { Color32::TRANSPARENT })
                        .stroke(Stroke::NONE)
                        .rounding(Rounding::same(9.0))
                        .min_size(Vec2::new(ui.available_width(), 42.0));
                    let resp = ui.add(b);
                    if resp.clicked() {
                        self.tela = t;
                    }
                }
                ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
                    ui.label(RichText::new(format!("{} {}", System::name().unwrap_or_default(), System::os_version().unwrap_or_default())).size(12.0).color(SUAVE));
                    ui.label(RichText::new(System::host_name().unwrap_or_default()).size(13.0).color(TEXTO));
                });
            });
    }

    fn painel_acoes(&mut self, ctx: &egui::Context) {
        egui::SidePanel::right("acoes")
            .exact_width(300.0)
            .resizable(false)
            .frame(egui::Frame::none().fill(FUNDO).inner_margin(egui::Margin { left: 0.0, right: 20.0, top: 22.0, bottom: 20.0 }))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("⚡").size(20.0).color(AZUL));
                    ui.label(RichText::new("Ações Rápidas").size(19.0).strong().color(TEXTO));
                });
                ui.add_space(6.0);
                let w = ui.available_width();
                let ocupado = self.ocupado();
                let tem_estado = self.estado.is_some();
                if botao(ui, "↺  Restaurar agora", "Volta tudo ao normal sem reiniciar", if tem_estado { AZUL } else { CARTAO }, w, !ocupado) {
                    self.restaurar();
                }
                ui.checkbox(&mut self.reabrir, RichText::new("Reabrir os apps fechados ao restaurar").size(13.0).color(SUAVE));
                ui.add_space(4.0);
                if botao(ui, "⟳  Reiniciar o computador", "O Windows volta 100% ao normal", CARTAO, w, !ocupado) {
                    self.confirmar = Confirmar::Reiniciar;
                }
                if botao(ui, "🗑  Limpar temporários", "Apaga arquivos das pastas Temp", CARTAO, w, !ocupado) {
                    let sh = self.sh.clone();
                    self.tela = Tela::Atividade;
                    std::thread::spawn(move || limpar_temporarios(sh));
                }
                if botao(ui, "🚀  Apps de inicialização", "Escolha o que abre com o Windows", CARTAO, w, true) {
                    abrir("ms-settings:startupapps");
                }
                ui.add_space(10.0);

                // estado atual
                let (fg, bg, tit, txt) = match &self.estado {
                    Some(e) => {
                        let min = (agora().saturating_sub(e.inicio)) / 60;
                        (
                            AZUL,
                            AZUL_CLARO,
                            format!("Perfil {} ativo", e.perfil),
                            format!(
                                "Há {} min · {} serviço(s) parado(s) · {} app(s) para reabrir{}",
                                min,
                                e.servicos.len(),
                                e.apps.len(),
                                if e.energia_original.is_some() { " · energia em Alto desempenho" } else { "" }
                            ),
                        )
                    }
                    None => (
                        VERDE,
                        VERDE_CLARO,
                        "Windows no estado normal".to_string(),
                        if self.reiniciado { "O PC foi reiniciado e tudo voltou ao padrão.".into() } else { "Nenhuma alteração temporária ativa.".into() },
                    ),
                };
                egui::Frame::none().fill(bg).stroke(Stroke::new(1.0_f32, fg.gamma_multiply(0.35))).rounding(Rounding::same(12.0)).inner_margin(egui::Margin::same(16.0)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new(tit).size(16.0).strong().color(fg));
                    ui.label(RichText::new(txt).size(13.0).color(TEXTO));
                });
                ui.add_space(10.0);
                egui::Frame::none().fill(AZUL_CLARO).rounding(Rounding::same(12.0)).inner_margin(egui::Margin::same(16.0)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new("ℹ  Importante").size(15.0).strong().color(AZUL));
                    ui.label(
                        RichText::new(
                            "O WinOpti nunca desativa nada de forma permanente: serviços são apenas parados e apps fechados. \
                             Ao reiniciar o PC (ou clicar em Restaurar) tudo volta como era. Ideal para quem usa o mesmo PC para jogar e trabalhar.",
                        )
                        .size(13.0)
                        .color(TEXTO),
                    );
                });
                if ocupado {
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(RichText::new("Trabalhando...").color(AZUL));
                    });
                }
            });
    }

    fn tela_inicio(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Olá! Vamos otimizar seu Windows").size(28.0).strong().color(TEXTO));
        ui.label(
            RichText::new("Pausa serviços e aplicativos desnecessários só durante esta sessão. Reiniciou, voltou ao normal.")
                .size(15.0)
                .color(SUAVE),
        );
        ui.add_space(12.0);

        cartao(ui, |ui| {
            titulo(ui, "▣", "Estado atual do Windows", "");
            let livre = 100.0 - (self.cpu * 0.5 + self.ram * 0.35 + self.disco * 0.15);
            ui.columns(5, |c| {
                medidor(&mut c[0], "Desempenho", livre.clamp(0.0, 100.0), "folga do sistema", true);
                medidor(&mut c[1], "Uso de CPU", self.cpu, "agora", false);
                medidor(&mut c[2], "Uso de Memória", self.ram, &self.ram_txt, false);
                medidor(&mut c[3], "Disco C:", self.disco, &self.disco_txt, false);
                let d = self.sh.diag.lock().unwrap().clone();
                let rodando = self.sh.diagnosticando.load(Ordering::SeqCst);
                c[4].vertical_centered(|ui| {
                    ui.label(RichText::new("Saúde (mapa)").size(13.5).color(SUAVE));
                    match &d {
                        Some(d) if !rodando => {
                            let (rect, _) = ui.allocate_exact_size(Vec2::new(110.0, 100.0), Sense::hover());
                            let cor = cor_nota(d.nota);
                            let p = ui.painter();
                            p.add(Shape::line(arco(rect.center(), 40.0, 0.0, std::f32::consts::TAU), Stroke::new(9.0_f32, TRILHO)));
                            let a0 = -std::f32::consts::FRAC_PI_2;
                            p.add(Shape::line(arco(rect.center(), 40.0, a0, a0 + std::f32::consts::TAU * (d.nota as f32 / 100.0).max(0.01)), Stroke::new(9.0_f32, cor)));
                            p.text(rect.center(), Align2::CENTER_CENTER, format!("{}", d.nota), FontId::proportional(24.0), TEXTO);
                            let crit = d.contar(Gravidade::Critico);
                            let alto = d.contar(Gravidade::Alto);
                            ui.label(RichText::new(if crit > 0 { "Ameaças!" } else if alto > 0 { "Atenção" } else { "Bom" }).size(15.0).strong().color(cor));
                            if ui.link(RichText::new(format!("{crit} críticos · {alto} altos")).size(12.0)).clicked() {
                                self.tela = Tela::Diagnostico;
                            }
                        }
                        _ => {
                            ui.add_space(30.0);
                            ui.spinner();
                            ui.label(RichText::new("Mapeando...").size(14.0).strong().color(AZUL));
                            ui.label(RichText::new(self.sh.etapa.lock().unwrap().clone()).size(11.5).color(SUAVE));
                        }
                    }
                });
            });
        });
        ui.add_space(12.0);
        self.cartao_mapa(ui);
        ui.add_space(12.0);

        cartao(ui, |ui| {
            titulo(ui, "⚡", "Perfis de Otimização", "Escolha como o computador vai ser usado agora.");
            let perfis = [
                (Perfil::Padrao, "🖥", "Padrão", "Windows normal. Desfaz tudo e reabre os apps.", ""),
                (Perfil::Otimizado, "⚙", "Otimizado", "Trabalho e uso diário: corta telemetria, agentes do fabricante e atualizadores.", "Recomendado"),
                (Perfil::Gamer, "🎮", "Gamer", "Máximo desempenho: para tudo que não é essencial e ativa Alto desempenho.", "Jogos"),
                (Perfil::Personalizado, "☰", "Personalizado", "Você escolhe item por item na aba Serviços e Aplicativos.", ""),
            ];
            ui.columns(4, |cols| {
                for (i, (p, ic, nome, desc, tag)) in perfis.iter().enumerate() {
                    let ui = &mut cols[i];
                    let sel = self.perfil == *p;
                    let r = egui::Frame::none()
                        .fill(if sel { AZUL_CLARO } else { CARTAO })
                        .stroke(Stroke::new(if sel { 2.0_f32 } else { 1.0 }, if sel { AZUL } else { BORDA }))
                        .rounding(Rounding::same(10.0))
                        .inner_margin(egui::Margin::same(12.0))
                        .show(ui, |ui| {
                            ui.set_min_height(150.0);
                            ui.set_width(ui.available_width());
                            ui.with_layout(Layout::top_down(Align::LEFT), |ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(if sel { "◉" } else { "○" }).size(16.0).color(if sel { AZUL } else { SUAVE }));
                                ui.label(RichText::new(*ic).size(20.0).color(AZUL));
                            });
                            ui.label(RichText::new(*nome).size(16.5).strong().color(if sel { AZUL } else { TEXTO }));
                            ui.label(RichText::new(*desc).size(12.5).color(SUAVE));
                            if !tag.is_empty() {
                                chip(ui, tag, VERDE, VERDE_CLARO);
                            }
                            });
                        });
                    let resp = ui.interact(r.response.rect, egui::Id::new(("perfil", i)), Sense::click());
                    if resp.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if resp.clicked() {
                        if *p == Perfil::Personalizado {
                            self.perfil = Perfil::Personalizado;
                            self.tela = Tela::Otimizacao;
                        } else {
                            self.escolher_perfil(*p);
                        }
                    }
                }
            });
            ui.add_space(8.0);
            let n = self.sel.iter().filter(|s| **s).count();
            ui.horizontal(|ui| {
                let w = (ui.available_width() - 10.0) / 2.0;
                let (t, s) = if self.perfil == Perfil::Padrao {
                    ("▶  Voltar ao Padrão", "Restaura serviços, energia e apps".to_string())
                } else {
                    ("▶  Otimizar Agora", format!("Aplicar perfil {} ({} itens)", self.perfil.nome(), n))
                };
                if botao(ui, t, &s, AZUL, w, !self.ocupado()) {
                    if self.perfil == Perfil::Padrao {
                        self.aplicar();
                    } else {
                        self.confirmar = Confirmar::Aplicar;
                    }
                }
                if botao(ui, "📄  Gerar Script", "Criar arquivo .BAT deste perfil", CARTAO, w, true) {
                    self.tela = Tela::Scripts;
                }
            });
        });
        ui.add_space(12.0);

        cartao(ui, |ui| {
            ui.label(RichText::new("O que o perfil selecionado faz").size(16.0).strong().color(TEXTO));
            let mut cats: Vec<(&str, usize)> = vec![];
            for (it, s) in ITENS.iter().zip(self.sel.iter()) {
                if *s {
                    if let Some(c) = cats.iter_mut().find(|c| c.0 == it.cat) {
                        c.1 += 1;
                    } else {
                        cats.push((it.cat, 1));
                    }
                }
            }
            if cats.is_empty() {
                ui.label(RichText::new("✔  Nenhuma alteração: Windows no modo normal.").color(VERDE));
            }
            ui.columns(2, |c| {
                for (i, (cat, n)) in cats.iter().enumerate() {
                    c[i % 2].label(RichText::new(format!("✔  {cat} ({n})")).size(14.0).color(TEXTO));
                }
            });
        });
    }

    fn tela_otimizacao(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Servicos e Aplicativos").size(26.0).strong().color(TEXTO));
        ui.label(RichText::new("Marque o que deve ser pausado. Os perfis preenchem esta lista; qualquer mudança vira \"Personalizado\".").color(SUAVE));
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            for p in [Perfil::Padrao, Perfil::Otimizado, Perfil::Gamer] {
                if ui.selectable_label(self.perfil == p, RichText::new(format!("Carregar perfil {}", p.nome())).size(14.0)).clicked() {
                    self.escolher_perfil(p);
                }
            }
            ui.label(RichText::new(format!("Atual: {}", self.perfil.nome())).strong().color(AZUL));
        });
        ui.add_space(8.0);
        let v = self.sh.varredura.lock().unwrap().clone();
        let mut mudou = false;
        for cat in CATEGORIAS {
            cartao(ui, |ui| {
                ui.label(RichText::new(*cat).size(16.0).strong().color(TEXTO));
                ui.add_space(2.0);
                for (idx, it) in ITENS.iter().enumerate().filter(|(_, i)| i.cat == *cat) {
                    ui.horizontal(|ui| {
                        if ui.checkbox(&mut self.sel[idx], RichText::new(it.nome).size(14.5).color(TEXTO)).changed() {
                            mudou = true;
                        }
                        if it.otimizado {
                            chip(ui, "Otimizado", AZUL, AZUL_CLARO);
                        }
                        if it.gamer {
                            chip(ui, "Gamer", VERDE, VERDE_CLARO);
                        }
                        if let Some(v) = &v {
                            let s = &v.itens[idx];
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label(RichText::new(&s.detalhe).size(12.5).color(if s.ativo { AMARELO } else { SUAVE }));
                            });
                        }
                    });
                    ui.indent(("d", idx), |ui| {
                        let mut t = it.desc.to_string();
                        if !it.servicos.is_empty() {
                            t += &format!("  Serviços: {}", it.servicos.join(", "));
                        }
                        ui.add(egui::Label::new(RichText::new(t).size(12.5).color(SUAVE)).wrap());
                    });
                }
            });
            ui.add_space(10.0);
        }
        if mudou {
            self.perfil = Perfil::Personalizado;
        }
        let w = ui.available_width();
        if botao(ui, "▶  Aplicar seleção", &format!("Perfil {}", self.perfil.nome()), AZUL, w, !self.ocupado()) {
            self.confirmar = Confirmar::Aplicar;
        }
    }

    fn tela_varredura(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Varredura e Segurança").size(26.0).strong().color(TEXTO));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let varrendo = self.sh.varrendo.load(Ordering::SeqCst);
                if ui.add_enabled(!varrendo, egui::Button::new(RichText::new("🔍  Vasculhar agora").size(15.0))).clicked() {
                    let sh = self.sh.clone();
                    std::thread::spawn(move || varrer(sh));
                }
                if varrendo {
                    ui.spinner();
                }
            });
        });
        ui.label(RichText::new("Procura mineradores, processos disfarçados, executáveis em pastas temporárias e o que mais pesa no PC.").color(SUAVE));
        ui.add_space(8.0);
        let v = match self.sh.varredura.lock().unwrap().clone() {
            Some(v) => v,
            None => {
                ui.spinner();
                return;
            }
        };

        cartao(ui, |ui| {
            titulo(ui, "🛡", "Segurança", &format!("{} processos analisados", v.total_proc));
            if v.alertas.is_empty() {
                ui.label(RichText::new("✔  Nenhum minerador, processo disfarçado ou bloqueio suspeito encontrado.").size(15.0).color(VERDE));
            }
            for a in &v.alertas {
                let (fg, bg) = if a.grave { (VERMELHO, VERMELHO_CLARO) } else { (AMARELO, AMARELO_CLARO) };
                egui::Frame::none().fill(bg).rounding(Rounding::same(8.0)).inner_margin(egui::Margin::same(8.0)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new(format!("{}  {}", if a.grave { "⚠" } else { "•" }, a.texto)).size(13.0).color(fg));
                });
            }
            ui.add_space(4.0);
            ui.label(
                RichText::new("Complemente com a Verificação completa do Kaspersky (com \"Detectar outros programas que podem ser usados por criminosos\" ligado).")
                    .size(12.5)
                    .color(SUAVE),
            );
        });
        ui.add_space(12.0);

        cartao(ui, |ui| {
            titulo(ui, "📊", "O que mais pesa agora (fora da lista do WinOpti)", "Marque para fechar também na próxima otimização. Programas do Windows e jogos não aparecem aqui.");
            if v.pesados.is_empty() {
                ui.label(RichText::new("Nada relevante.").color(SUAVE));
            }
            egui::Grid::new("pesados").striped(true).num_columns(5).spacing([16.0, 6.0]).show(ui, |ui| {
                ui.label(RichText::new("Fechar").strong());
                ui.label(RichText::new("Programa").strong());
                ui.label(RichText::new("RAM").strong());
                ui.label(RichText::new("CPU").strong());
                ui.label(RichText::new("Local").strong());
                ui.end_row();
                for p in &v.pesados {
                    let mut m = self.extras.contains(&p.stem);
                    if ui.checkbox(&mut m, "").changed() {
                        if m {
                            self.extras.insert(p.stem.clone());
                        } else {
                            self.extras.remove(&p.stem);
                        }
                    }
                    ui.label(format!("{}{}", p.nome, if p.qtd > 1 { format!(" (x{})", p.qtd) } else { String::new() }));
                    ui.label(format!("{:.0} MB", p.ram_mb));
                    ui.label(format!("{:.1}%", p.cpu));
                    let cam = if p.caminho.len() > 70 { format!("...{}", &p.caminho[p.caminho.len() - 67..]) } else { p.caminho.clone() };
                    ui.label(RichText::new(cam).size(12.0).color(SUAVE));
                    ui.end_row();
                }
            });
        });
        ui.add_space(12.0);

        cartao(ui, |ui| {
            titulo(ui, "✔", "Itens do catálogo encontrados neste PC", &format!("O perfil Gamer libera cerca de {:.0} MB de RAM agora.", v.economia_mb));
            for (it, s) in ITENS.iter().zip(v.itens.iter()) {
                if s.ativo {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("●").color(AMARELO));
                        ui.label(RichText::new(it.nome).color(TEXTO));
                        ui.label(RichText::new(&s.detalhe).size(12.5).color(SUAVE));
                    });
                }
            }
        });
    }

    fn tela_scripts(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Gerar Script (.bat)").size(26.0).strong().color(TEXTO));
        ui.label(RichText::new("Cria dois arquivos para usar sem abrir o WinOpti: um aplica o perfil e o outro restaura (ou reinicia o PC).").color(SUAVE));
        ui.add_space(10.0);
        cartao(ui, |ui| {
            titulo(ui, "📄", "Perfil do script", "");
            ui.horizontal(|ui| {
                for p in [Perfil::Otimizado, Perfil::Gamer, Perfil::Personalizado] {
                    if ui.selectable_label(self.perfil == p, RichText::new(p.nome()).size(15.0)).clicked() {
                        if p == Perfil::Personalizado {
                            self.perfil = p;
                        } else {
                            self.escolher_perfil(p);
                        }
                    }
                }
            });
            ui.label(RichText::new(format!("{} itens selecionados. Para personalizar, use a aba Serviços e Aplicativos.", self.sel.iter().filter(|s| **s).count())).size(13.0).color(SUAVE));
            ui.add_space(8.0);
            ui.label(RichText::new("Salvar em:").strong());
            ui.horizontal(|ui| {
                let mut t = self.pasta_bat.to_string_lossy().into_owned();
                if ui.add(egui::TextEdit::singleline(&mut t).desired_width(ui.available_width() - 130.0)).changed() {
                    self.pasta_bat = PathBuf::from(t);
                }
                if ui.button("Abrir pasta").clicked() {
                    abrir(&self.pasta_bat.to_string_lossy());
                }
            });
            ui.add_space(8.0);
            let w = ui.available_width();
            let perfil = if self.perfil == Perfil::Padrao { Perfil::Otimizado } else { self.perfil };
            if botao(ui, "📄  Gerar arquivos .BAT", &format!("WinOpti-{}.bat + WinOpti-Restaurar.bat", perfil.nome()), AZUL, w, true) {
                self.msg_bat = Some(match bat::gerar(&self.pasta_bat, perfil.nome(), &self.sel) {
                    Ok((a, b)) => (true, format!("Criados:\n{}\n{}", a.display(), b.display())),
                    Err(e) => (false, format!("Não foi possível salvar: {e}")),
                });
            }
            if let Some((ok, m)) = &self.msg_bat {
                ui.label(RichText::new(m).color(if *ok { VERDE } else { VERMELHO }));
            }
        });
        ui.add_space(12.0);
        cartao(ui, |ui| {
            titulo(ui, "ℹ", "Como usar os scripts", "");
            for l in [
                "1. Clique duas vezes em WinOpti-<Perfil>.bat e aceite o pedido de administrador.",
                "2. Jogue ou trabalhe normalmente.",
                "3. Para voltar: reinicie o PC ou rode WinOpti-Restaurar.bat (ele pergunta se quer reiniciar).",
                "Os scripts usam apenas comandos do Windows (net stop, taskkill, powercfg). Nada e desativado permanentemente.",
            ] {
                ui.label(RichText::new(l).size(14.0).color(TEXTO));
            }
        });
    }

    fn tela_atividade(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Registro de Atividade").size(26.0).strong().color(TEXTO));
        ui.add_space(8.0);
        cartao(ui, |ui| {
            let log = self.sh.log.lock().unwrap().clone();
            if log.is_empty() {
                ui.label(RichText::new("Nenhuma ação executada ainda.").color(SUAVE));
            }
            for l in &log {
                let cor = if l.starts_with("[OK]") {
                    VERDE
                } else if l.starts_with("[--]") {
                    AMARELO
                } else if l.starts_with("===") {
                    AZUL
                } else {
                    TEXTO
                };
                ui.label(RichText::new(l).size(13.5).family(egui::FontFamily::Monospace).color(cor));
            }
            if self.ocupado() {
                ui.spinner();
            }
        });
    }

    fn tela_sobre(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Como o WinOpti funciona").size(26.0).strong().color(TEXTO));
        ui.add_space(8.0);
        cartao(ui, |ui| {
            for (t, d) in [
                ("Temporário por design", "Serviços são apenas PARADOS (o tipo de inicialização nunca muda) e apps são FECHADOS. Ao reiniciar, o Windows sobe exatamente como antes."),
                ("Padrão", "Restaura tudo agora: inicia os serviços parados, volta o plano de energia e reabre os apps que estavam abertos."),
                ("Otimizado", "Para trabalho e uso diário. Corta telemetria, agentes do fabricante (Dell, HP), atualizadores, widgets e Phone Link. Mantém WhatsApp, Teams, nuvem e impressão."),
                ("Gamer", "Tudo do Otimizado + WhatsApp, Teams, OneDrive/Drive, ChatGPT/Claude, indexação, SysMain, impressão, Windows Update e plano Alto desempenho. Discord, Spotify, Steam e launchers continuam abertos."),
                ("Segurança", "Nunca fecha processos do Windows, antivírus (Kaspersky, Defender), drivers ou launchers de jogos. Bancos de dados de ERP/PDV só param se você marcar."),
                ("Plano de energia", "Se trocado, o WinOpti agenda a volta do plano original para o próximo login (RunOnce), então reiniciar sempre restaura."),
            ] {
                ui.label(RichText::new(t).size(16.0).strong().color(AZUL));
                ui.label(RichText::new(d).size(14.0).color(TEXTO));
                ui.add_space(6.0);
            }
        });
    }

    fn dialogos(&mut self, ctx: &egui::Context) {
        let (titulo, texto) = match self.confirmar {
            Confirmar::Nada => return,
            Confirmar::Reiniciar => ("Reiniciar o computador?", "Salve seus trabalhos. O PC reinicia em 10 segundos e volta ao estado normal.".to_string()),
            Confirmar::Aplicar => (
                "Aplicar otimização?",
                format!(
                    "Perfil {}: {} itens do catálogo{} serão pausados até reiniciar ou restaurar.\n{}\nSalve o que estiver aberto nos apps que serão fechados.",
                    self.perfil.nome(),
                    self.sel.iter().filter(|s| **s).count(),
                    if self.extras.len() + self.extras_serv.len() > 0 {
                        format!(" + {} item(ns) marcados no Diagnóstico", self.extras.len() + self.extras_serv.len())
                    } else {
                        String::new()
                    },
                    match self.sh.diag.lock().unwrap().as_ref() {
                        Some(d) => format!(
                            "Mapa: nota {} · {} críticos · {} altos. Ameaças (mineradores, adware) devem ser removidas, não só pausadas.",
                            d.nota,
                            d.contar(Gravidade::Critico),
                            d.contar(Gravidade::Alto)
                        ),
                        None => "O mapa ainda não terminou. Recomendo esperar o Diagnóstico.".into(),
                    }
                ),
            ),
        };
        let mut fechar = false;
        let mut confirmar = false;
        egui::Window::new(RichText::new(titulo).size(17.0).strong())
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .fixed_size([480.0, 160.0])
            .show(ctx, |ui| {
                ui.label(RichText::new(texto).size(14.0));
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.add(egui::Button::new(RichText::new("Confirmar").color(Color32::WHITE)).fill(AZUL).min_size(Vec2::new(120.0, 34.0))).clicked() {
                        confirmar = true;
                    }
                    if ui.add(egui::Button::new("Cancelar").min_size(Vec2::new(100.0, 34.0))).clicked() {
                        fechar = true;
                    }
                });
            });
        if confirmar {
            match self.confirmar {
                Confirmar::Reiniciar => {
                    let _ = cmd("shutdown", &["/r", "/t", "10", "/c", "WinOpti: reiniciando para voltar ao normal"]);
                }
                Confirmar::Aplicar => self.aplicar(),
                Confirmar::Nada => {}
            }
            fechar = true;
        }
        if fechar {
            self.confirmar = Confirmar::Nada;
        }
    }
}

fn main() -> eframe::Result<()> {
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("WinOpti - Otimizador de Desempenho do Windows")
            .with_inner_size([1400.0, 900.0])
            .with_min_inner_size([1150.0, 700.0]),
        ..Default::default()
    };
    eframe::run_native("WinOpti", opts, Box::new(|cc| Ok(Box::new(App::new(cc)))))
}
