//! Telas do Diagnostico profundo (mapa de ameacas).

use super::*;
use crate::diagnostico::{self as dg, Achado, ProcResumo};

const LARANJA: Color32 = Color32::from_rgb(222, 110, 30);
const LARANJA_CLARO: Color32 = Color32::from_rgb(253, 240, 228);

pub fn cor_grav(g: Gravidade) -> (Color32, Color32) {
    match g {
        Gravidade::Critico => (VERMELHO, VERMELHO_CLARO),
        Gravidade::Alto => (LARANJA, LARANJA_CLARO),
        Gravidade::Medio => (AMARELO, AMARELO_CLARO),
        Gravidade::Baixo => (AZUL, AZUL_CLARO),
        Gravidade::Info => (SUAVE, TRILHO),
    }
}

pub fn cor_nota(n: u8) -> Color32 {
    if n >= 80 {
        VERDE
    } else if n >= 55 {
        AMARELO
    } else {
        VERMELHO
    }
}

fn chave(a: &Achado) -> String {
    format!("{}|{}", a.titulo, a.detalhe)
}

impl App {
    /// Cartao da tela inicial: um bloco por categoria de "inimigo".
    pub fn cartao_mapa(&mut self, ui: &mut egui::Ui) {
        let d = self.sh.diag.lock().unwrap().clone();
        let rodando = self.sh.diagnosticando.load(Ordering::SeqCst);
        cartao(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("🔎").size(20.0).color(AZUL));
                ui.label(RichText::new("Mapa dos inimigos do PC").size(19.0).strong().color(TEXTO));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.add_enabled(!rodando, egui::Button::new("Mapear novamente")).clicked() {
                        let sh = self.sh.clone();
                        std::thread::spawn(move || dg::executar(sh));
                    }
                    if ui.button("Ver mapa completo").clicked() {
                        self.tela = Tela::Diagnostico;
                    }
                });
            });
            ui.label(
                RichText::new("Antes de otimizar, o WinOpti procura o que está escondido: mineradores, programas iniciando invisíveis, tarefas ocultas, serviços problemáticos e a saúde da bateria.")
                    .size(13.0)
                    .color(SUAVE),
            );
            ui.add_space(6.0);
            if rodando || d.is_none() {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(RichText::new(self.sh.etapa.lock().unwrap().clone()).color(AZUL));
                });
                return;
            }
            let d = d.unwrap();
            let cats = Categoria::TODAS;
            for linha in cats.chunks(3) {
                ui.columns(3, |cols| {
                    for (i, c) in linha.iter().enumerate() {
                        let ui = &mut cols[i];
                        let itens: Vec<&Achado> = d.da_categoria(*c).into_iter().filter(|a| a.grav <= Gravidade::Medio).collect();
                        let pior = itens.iter().map(|a| a.grav).min();
                        let (fg, bg) = match pior {
                            Some(g) => cor_grav(g),
                            None => (VERDE, VERDE_CLARO),
                        };
                        let r = egui::Frame::none().fill(bg).rounding(Rounding::same(10.0)).inner_margin(egui::Margin::same(10.0)).show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.with_layout(Layout::top_down(Align::LEFT), |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(c.icone()).size(18.0).color(fg));
                                    ui.label(RichText::new(c.nome()).size(14.0).strong().color(TEXTO));
                                });
                                let txt = if *c == Categoria::Bateria && !d.bateria.tem {
                                    "Sem bateria (desktop)".to_string()
                                } else if itens.is_empty() {
                                    "✔ Nada preocupante".to_string()
                                } else {
                                    format!("{} problema(s) · pior: {}", itens.len(), pior.unwrap().nome())
                                };
                                ui.label(RichText::new(txt).size(12.5).color(fg));
                            });
                        });
                        let resp = ui.interact(r.response.rect, egui::Id::new(("mapa", *c as u8)), Sense::click());
                        if resp.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        if resp.clicked() {
                            self.filtro_cat = Some(*c);
                            self.aba_diag = if *c == Categoria::Bateria { 3 } else { 0 };
                            self.tela = Tela::Diagnostico;
                        }
                    }
                });
            }
        });
    }

    pub fn tela_diagnostico(&mut self, ui: &mut egui::Ui) {
        let rodando = self.sh.diagnosticando.load(Ordering::SeqCst);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Diagnóstico profundo").size(26.0).strong().color(TEXTO));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let d = self.sh.diag.lock().unwrap().clone();
                if ui.add_enabled(d.is_some() && !rodando, egui::Button::new("💾  Salvar relatório (.txt)")).clicked() {
                    if let Some(d) = d {
                        let arq = crate::bat::area_de_trabalho().join(format!("WinOpti-Relatorio-{}.txt", agora()));
                        self.msg_diag = Some(match std::fs::write(&arq, format!("\u{FEFF}{}", dg::relatorio_txt(&d))) {
                            Ok(_) => {
                                abrir(&arq.to_string_lossy());
                                format!("Relatório salvo em {}", arq.display())
                            }
                            Err(e) => format!("Não foi possível salvar: {e}"),
                        });
                    }
                }
                if ui.add_enabled(!rodando, egui::Button::new("🔄  Mapear novamente")).clicked() {
                    self.feitos.clear();
                    let sh = self.sh.clone();
                    std::thread::spawn(move || dg::executar(sh));
                }
            });
        });
        ui.label(
            RichText::new("Somente leitura: nada é alterado até você clicar em uma ação. Desativar inicialização/tarefa é reversível na aba Desativados.")
                .color(SUAVE),
        );
        if let Some(m) = &self.msg_diag {
            ui.label(RichText::new(m).color(VERDE));
        }
        ui.add_space(8.0);

        let d = match (rodando, self.sh.diag.lock().unwrap().clone()) {
            (false, Some(d)) => d,
            _ => {
                cartao(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(RichText::new("Mapeando o computador...").size(16.0).strong().color(AZUL));
                    });
                    ui.label(RichText::new(self.sh.etapa.lock().unwrap().clone()).color(SUAVE));
                    ui.label(RichText::new("Leva de 10 a 40 segundos. Evite abrir programas pesados enquanto isso para a medição ficar correta.").size(12.5).color(SUAVE));
                });
                return;
            }
        };

        // resumo
        cartao(ui, |ui| {
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(Vec2::new(96.0, 96.0), Sense::hover());
                let cor = cor_nota(d.nota);
                let p = ui.painter();
                p.add(Shape::line(arco(rect.center(), 38.0, 0.0, std::f32::consts::TAU), Stroke::new(9.0_f32, TRILHO)));
                let a0 = -std::f32::consts::FRAC_PI_2;
                p.add(Shape::line(arco(rect.center(), 38.0, a0, a0 + std::f32::consts::TAU * (d.nota as f32 / 100.0).max(0.01)), Stroke::new(9.0_f32, cor)));
                p.text(rect.center(), Align2::CENTER_CENTER, format!("{}", d.nota), FontId::proportional(26.0), TEXTO);
                ui.vertical(|ui| {
                    ui.set_max_width((ui.available_width() - 270.0).max(200.0));
                    ui.label(RichText::new("Nota de saúde do sistema").size(17.0).strong().color(TEXTO));
                    ui.horizontal_wrapped(|ui| {
                        for g in [Gravidade::Critico, Gravidade::Alto, Gravidade::Medio, Gravidade::Baixo] {
                            let (fg, bg) = cor_grav(g);
                            chip(ui, &format!("{} {}", d.contar(g), g.nome()), fg, bg);
                        }
                    });
                    ui.add(egui::Label::new(
                        RichText::new(format!(
                            "{} processos · {} serviços · {} tarefas agendadas · {} itens de inicialização · RAM {:.0}% de {:.0} GB · {:.0}s",
                            d.total_proc,
                            d.total_servicos,
                            d.total_tarefas,
                            d.inicio.len(),
                            d.ram_uso_pct,
                            d.ram_total_gb,
                            d.segundos
                        ))
                        .size(12.5)
                        .color(SUAVE),
                    ).wrap());
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if botao(ui, "✚  Incluir recomendados", "Críticos e altos entram na otimização", AZUL, 250.0, true) {
                        let mut n = 0;
                        for a in d.achados.iter().filter(|a| a.grav <= Gravidade::Alto) {
                            match &a.acao {
                                Acao::FecharProcesso(s) => n += self.extras.insert(s.clone()) as usize,
                                Acao::PararServico(s) => n += self.extras_serv.insert(s.clone()) as usize,
                                _ => {}
                            }
                        }
                        self.msg_diag = Some(format!("{n} item(ns) adicionados ao próximo \"Otimizar Agora\" (temporário). Itens de inicialização/tarefas: use os botões de cada um."));
                    }
                });
            });
        });
        ui.add_space(10.0);

        // abas
        ui.horizontal(|ui| {
            let abas = ["⚠  Problemas encontrados", "📈  Consumo por processo", "🚀  Inicialização", "🔋  Bateria", "↺  Desativados"];
            for (i, t) in abas.iter().enumerate() {
                if ui.selectable_label(self.aba_diag == i as u8, RichText::new(*t).size(15.0)).clicked() {
                    self.aba_diag = i as u8;
                }
            }
        });
        ui.add_space(6.0);
        match self.aba_diag {
            0 => self.aba_problemas(ui, &d),
            1 => self.aba_processos(ui, &d.top),
            2 => self.aba_inicio(ui, &d),
            3 => self.aba_bateria(ui, &d),
            _ => self.aba_desativados(ui),
        }
    }

    fn aba_problemas(&mut self, ui: &mut egui::Ui, d: &Diagnostico) {
        ui.horizontal_wrapped(|ui| {
            if ui.selectable_label(self.filtro_cat.is_none(), "Todas").clicked() {
                self.filtro_cat = None;
            }
            for c in Categoria::TODAS {
                let n = d.da_categoria(c).iter().filter(|a| self.mostrar_info || a.grav != Gravidade::Info).count();
                if n == 0 {
                    continue;
                }
                if ui.selectable_label(self.filtro_cat == Some(c), format!("{} {} ({n})", c.icone(), c.nome())).clicked() {
                    self.filtro_cat = Some(c);
                }
            }
            ui.checkbox(&mut self.mostrar_info, "Mostrar informativos");
        });
        ui.add_space(6.0);
        let lista: Vec<&Achado> = d
            .achados
            .iter()
            .filter(|a| self.filtro_cat.map(|c| c == a.cat).unwrap_or(true))
            .filter(|a| self.mostrar_info || a.grav != Gravidade::Info)
            .collect();
        if lista.is_empty() {
            cartao(ui, |ui| {
                ui.label(RichText::new("✔  Nada encontrado nesta categoria.").size(15.0).color(VERDE));
            });
        }
        for (idx, a) in lista.iter().enumerate() {
            let (fg, bg) = cor_grav(a.grav);
            let k = chave(a);
            let feito = self.feitos.contains(&k);
            egui::Frame::none()
                .fill(CARTAO)
                .stroke(Stroke::new(1.0_f32, BORDA))
                .rounding(Rounding::same(10.0))
                .inner_margin(egui::Margin::same(12.0))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        chip(ui, a.grav.nome(), fg, bg);
                        chip(ui, a.cat.nome(), SUAVE, TRILHO);
                        ui.add(egui::Label::new(RichText::new(&a.titulo).size(15.0).strong().color(TEXTO)).truncate());
                    });
                    if !a.detalhe.is_empty() {
                        ui.add(egui::Label::new(RichText::new(&a.detalhe).size(12.5).family(egui::FontFamily::Monospace).color(SUAVE)).wrap());
                    }
                    ui.add(egui::Label::new(RichText::new(&a.porque).size(13.0).color(TEXTO)).wrap());
                    ui.horizontal(|ui| {
                        if feito {
                            ui.label(RichText::new("✔ Feito").color(VERDE));
                        }
                        let rotulo = match &a.acao {
                            Acao::FecharProcesso(_) => Some("Fechar agora"),
                            Acao::PararServico(_) => Some("Parar até reiniciar"),
                            Acao::DesativarInicio(_) => Some("Não iniciar com o Windows"),
                            Acao::DesativarTarefa(_) => Some("Desativar tarefa"),
                            _ => None,
                        };
                        if let Some(r) = rotulo {
                            if !feito && ui.add_enabled(!self.ocupado(), egui::Button::new(RichText::new(r).color(Color32::WHITE)).fill(AZUL)).clicked() {
                                let sh = self.sh.clone();
                                let acao = a.acao.clone();
                                self.feitos.insert(k.clone());
                                std::thread::spawn(move || {
                                    dg::executar_acao(&sh, &acao);
                                });
                            }
                        }
                        match &a.acao {
                            Acao::FecharProcesso(s) => {
                                let mut m = self.extras.contains(s);
                                if ui.checkbox(&mut m, "Incluir na otimização").changed() {
                                    if m { self.extras.insert(s.clone()); } else { self.extras.remove(s); }
                                }
                            }
                            Acao::PararServico(s) => {
                                let mut m = self.extras_serv.contains(s);
                                if ui.checkbox(&mut m, "Incluir na otimização").changed() {
                                    if m { self.extras_serv.insert(s.clone()); } else { self.extras_serv.remove(s); }
                                }
                            }
                            _ => {}
                        }
                        if !a.caminho.is_empty() && std::path::Path::new(&a.caminho).exists() && ui.button("📂 Abrir local").clicked() {
                            let _ = dg::executar_acao(&self.sh, &Acao::AbrirLocal(a.caminho.clone()));
                        }
                        if !a.caminho.is_empty() && ui.button("🔎 Pesquisar na web").clicked() {
                            let nome = std::path::Path::new(&a.caminho).file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
                            abrir(&format!("https://www.google.com/search?q={}+o+que+e+seguro", nome.replace(' ', "+")));
                        }
                    });
                });
            ui.add_space(6.0);
            let _ = idx;
        }
    }

    fn aba_processos(&mut self, ui: &mut egui::Ui, top: &[ProcResumo]) {
        cartao(ui, |ui| {
            titulo(ui, "📈", "O que mais consome agora", "Medição de 2,5 segundos. Marque para fechar na próxima otimização (temporário).");
            egui::Grid::new("top").striped(true).num_columns(8).spacing([14.0, 6.0]).show(ui, |ui| {
                for h in ["Fechar", "Programa", "RAM", "CPU", "GPU", "Disco", "Assinatura", "Local"] {
                    ui.label(RichText::new(h).strong());
                }
                ui.end_row();
                for p in top {
                    let protegido = PROTEGIDOS.contains(&p.stem.as_str()) || p.caminho.to_lowercase().starts_with("c:\\windows\\");
                    if protegido {
                        ui.label(RichText::new("🔒").color(SUAVE));
                    } else {
                        let mut m = self.extras.contains(&p.stem);
                        if ui.checkbox(&mut m, "").changed() {
                            if m { self.extras.insert(p.stem.clone()); } else { self.extras.remove(&p.stem); }
                        }
                    }
                    ui.label(format!("{}{}", p.nome, if p.qtd > 1 { format!(" (x{})", p.qtd) } else { String::new() }));
                    ui.label(RichText::new(format!("{:.0} MB", p.ram_mb)).color(if p.ram_mb > 1500.0 { LARANJA } else { TEXTO }));
                    ui.label(RichText::new(format!("{:.1}%", p.cpu)).color(if p.cpu > 15.0 { LARANJA } else { TEXTO }));
                    ui.label(RichText::new(format!("{:.0}%", p.gpu)).color(if p.gpu > 25.0 { LARANJA } else { TEXTO }));
                    ui.label(RichText::new(format!("{:.1} MB/s", p.disco_mbs)).color(if p.disco_mbs > 8.0 { LARANJA } else { TEXTO }));
                    match p.assinado {
                        Some(true) => ui.label(RichText::new("✔ assinado").color(VERDE)),
                        Some(false) => ui.label(RichText::new("✖ sem assinatura").color(LARANJA)),
                        None => ui.label(RichText::new("—").color(SUAVE)),
                    };
                    ui.label(RichText::new(dg::truncar(&p.caminho, 60)).size(12.0).color(SUAVE));
                    ui.end_row();
                }
            });
        });
    }

    fn aba_inicio(&mut self, ui: &mut egui::Ui, d: &Diagnostico) {
        cartao(ui, |ui| {
            titulo(
                ui,
                "🚀",
                "Tudo que inicia com o Windows",
                "Inclui o que o Gerenciador de Tarefas não mostra (RunOnce, 32 bits, pasta comum). Desativar aqui é igual ao Gerenciador de Tarefas e pode ser revertido.",
            );
            let atuais = dg::listar_inicializacao();
            for e in &atuais {
                let ativo = e.ativo;
                ui.horizontal(|ui| {
                    chip(ui, if ativo { "ATIVO" } else { "desativado" }, if ativo { LARANJA } else { SUAVE }, if ativo { LARANJA_CLARO } else { TRILHO });
                    ui.label(RichText::new(&e.nome).strong().color(TEXTO));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if !e.origem.contains("RunOnce") {
                            if ativo {
                                if ui.button("Não iniciar").clicked() {
                                    dg::executar_acao(&self.sh, &Acao::DesativarInicio(e.clone()));
                                }
                            } else if ui.button("Reativar").clicked() {
                                dg::definir_inicio(e, true);
                                let lista = dg::listar_desativados();
                                if let Some(x) = lista.iter().find(|x| x.tipo == "inicio" && x.nome == e.nome) {
                                    dg::reativar(&self.sh, x);
                                }
                            }
                        }
                    });
                });
                ui.indent(("ini", &e.nome), |ui| {
                    ui.add(egui::Label::new(RichText::new(&e.comando).size(12.0).family(egui::FontFamily::Monospace).color(SUAVE)).wrap());
                });
            }
            let _ = d;
        });
    }

    fn aba_bateria(&mut self, ui: &mut egui::Ui, d: &Diagnostico) {
        let b = &d.bateria;
        cartao(ui, |ui| {
            titulo(ui, "🔋", "Bateria e energia", "");
            if !b.tem {
                ui.label(RichText::new("Nenhuma bateria encontrada: computador de mesa ou bateria removida.").size(15.0).color(SUAVE));
                return;
            }
            ui.columns(4, |c| {
                medidor(&mut c[0], "Saúde da bateria", b.saude().unwrap_or(0.0), "capacidade restante", true);
                medidor(&mut c[1], "Carga atual", b.carga as f32, if b.na_tomada { "na tomada" } else { "na bateria" }, true);
                c[2].vertical(|ui| {
                    ui.label(RichText::new("Capacidade").size(13.5).color(SUAVE));
                    ui.label(RichText::new(format!("{:.1} Wh", b.atual_mwh as f64 / 1000.0)).size(22.0).strong().color(TEXTO));
                    ui.label(RichText::new(format!("original: {:.1} Wh", b.projetada_mwh as f64 / 1000.0)).size(12.5).color(SUAVE));
                    if let Some(cy) = b.ciclos {
                        ui.label(RichText::new(format!("{cy} ciclos de carga")).size(12.5).color(SUAVE));
                    }
                });
                c[3].vertical(|ui| {
                    ui.label(RichText::new("Autonomia estimada").size(13.5).color(SUAVE));
                    match b.autonomia_min {
                        Some(m) => ui.label(RichText::new(format!("{}h{:02}", m / 60, m % 60)).size(22.0).strong().color(TEXTO)),
                        None => ui.label(RichText::new("—").size(22.0).color(SUAVE)),
                    };
                    if let Some(m) = b.autonomia_projetada_min {
                        ui.label(RichText::new(format!("quando nova: {}h{:02}", m / 60, m % 60)).size(12.5).color(SUAVE));
                    }
                    ui.label(RichText::new(format!("{} {}", b.fabricante, b.quimica)).size(12.0).color(SUAVE));
                });
            });
            ui.add_space(6.0);
            for a in d.da_categoria(Categoria::Bateria) {
                let (fg, _) = cor_grav(a.grav);
                ui.label(RichText::new(format!("• {}", a.titulo)).strong().color(fg));
                if !a.detalhe.is_empty() {
                    ui.label(RichText::new(&a.detalhe).size(12.5).color(SUAVE));
                }
                ui.add(egui::Label::new(RichText::new(&a.porque).size(13.0).color(TEXTO)).wrap());
            }
            ui.add_space(6.0);
            ui.label(RichText::new("Dicas para a bateria durar mais").strong().color(AZUL));
            for t in [
                "Use o limite de carga em 80% (Dell Power Manager/Optimizer: \"Uso principal na tomada\") se o notebook fica quase sempre ligado na tomada.",
                "Para jogar, sempre na tomada: na bateria a placa de vídeo perde boa parte do desempenho.",
                "Calor é o maior inimigo: use em superfície firme e limpe as saídas de ar.",
                "Evite descarregar até 0% com frequência.",
            ] {
                ui.label(RichText::new(format!("✔ {t}")).size(13.0).color(TEXTO));
            }
            if ui.button("Abrir relatório completo de bateria do Windows").clicked() {
                let arq = std::env::temp_dir().join("relatorio-bateria.html");
                let _ = cmd("powercfg", &["/batteryreport", "/output", &arq.to_string_lossy()]);
                abrir(&arq.to_string_lossy());
            }
        });
    }

    fn aba_desativados(&mut self, ui: &mut egui::Ui) {
        cartao(ui, |ui| {
            titulo(ui, "↺", "Itens desativados pelo WinOpti", "Inicialização e tarefas desativadas aqui ficam assim até você reativar.");
            let lista = dg::listar_desativados();
            if lista.is_empty() {
                ui.label(RichText::new("Nenhum item desativado.").color(SUAVE));
            }
            for x in &lista {
                ui.horizontal(|ui| {
                    chip(ui, if x.tipo == "inicio" { "Inicialização" } else { "Tarefa" }, AZUL, AZUL_CLARO);
                    ui.label(RichText::new(&x.nome).color(TEXTO));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button("Reativar").clicked() {
                            dg::reativar(&self.sh, x);
                        }
                    });
                });
            }
            if !lista.is_empty() && ui.button("Reativar tudo").clicked() {
                for x in &lista {
                    dg::reativar(&self.sh, x);
                }
            }
        });
    }
}
