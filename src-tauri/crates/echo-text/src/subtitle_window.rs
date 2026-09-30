//! Что именно транскрибировать для живых субтитров — и что уже можно не трогать.
//!
//! # Зачем это существует
//!
//! Задача субтитров в `actions.rs` звала `transcribe(peek_recording())`, то есть
//! прогоняла ВЕСЬ буфер записи заново каждые `subtitle_refresh_ms` (по умолчанию
//! 300 мс). Замер на установленном билде (parakeet-tdt-0.6b-v3, 3 прогона на длину,
//! медиана; в каждом прогоне есть постоянная загрузка модели, поэтому смысл имеет
//! ПРИРОСТ, где она сокращается):
//!
//! | буфер   | тик     |
//! |---------|---------|
//! | 12.4 с  | 5.46 с  |
//! | 31.1 с  | 7.72 с  |
//! | 62.2 с  | 13.75 с |
//!
//! Прирост 30→60 = +6.03 с на +31 с аудио ⇒ ≈0.19 с счёта на секунду речи, константа
//! загрузки ≈3.4 с. То есть чистая транскрипция минутного буфера ≈10 с при интервале
//! 300 мс — тик считается в ~33 раза дольше, чем отведено. Цикл не «иногда отстаёт»:
//! с какого-то момента он крутится вплотную и показывает всё более старый текст.
//!
//! # Что делает этот модуль
//!
//! Держит границу «до сюда уже расшифровано и меняться не будет» и говорит вызывающему,
//! какой отрезок буфера отдавать движку. Речь режется по паузам, которые находит VAD
//! (у Echo он нейросетевой — Silero под `SmoothedVad`), поэтому модуль принимает не
//! аудио, а уже готовые пофреймовые решения `voiced: bool`. Благодаря этому он
//! Tauri-free и cpal-free, живёт в `echo-text` и проверяется `cargo test` на Windows —
//! в отличие от app-крейта, который локально не собирается вовсе (инвариант в AGENTS.md).
//!
//! Семантический конец реплики (модель понимает, что фраза закончена) для этой задачи
//! НЕ нужен — см. интейк #130: чтобы не перетранскрибировать буфер, достаточно пауз,
//! а они, в отличие от EOU-модели, языконезависимы.

/// Что транскрибировать на этом тике.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TranscribePlan {
    /// Начало отрезка в сэмплах (включительно).
    pub start: usize,
    /// Конец отрезка в сэмплах (исключительно).
    pub end: usize,
    /// Отрезок заканчивается на границе реплики: результат можно зафиксировать
    /// через [`SubtitleWindow::commit`] и больше к нему не возвращаться.
    pub closes_utterance: bool,
}

impl TranscribePlan {
    /// Пустой отрезок — транскрибировать нечего (движок не звать вовсе).
    pub fn is_empty(&self) -> bool {
        self.end <= self.start
    }
}

/// Скользящее окно живых субтитров.
#[derive(Debug)]
pub struct SubtitleWindow {
    frame_samples: usize,
    silence_frames_to_close: usize,

    frames_seen: usize,
    /// Длина текущей хвостовой серии тишины во фреймах.
    silence_run: usize,
    /// Граница в сэмплах, до которой всё уже зафиксировано.
    committed_samples: usize,
    /// Найденная, но ещё не зафиксированная граница (в сэмплах).
    pending_boundary: Option<usize>,
    committed_text: String,
}

impl SubtitleWindow {
    /// `frame_samples` — размер фрейма VAD в сэмплах; `silence_frames_to_close` —
    /// сколько подряд тихих фреймов считать концом реплики.
    ///
    /// Оба параметра обязаны быть > 0: нулевой фрейм сделал бы границы бессмысленными
    /// (любой индекс сэмпла = 0), а нулевой порог закрывал бы реплику на первой же
    /// тишине внутри фразы. Вместо тихой подмены значений — жёсткая нижняя граница 1.
    pub fn new(frame_samples: usize, silence_frames_to_close: usize) -> Self {
        Self {
            frame_samples: frame_samples.max(1),
            silence_frames_to_close: silence_frames_to_close.max(1),
            frames_seen: 0,
            silence_run: 0,
            committed_samples: 0,
            pending_boundary: None,
            committed_text: String::new(),
        }
    }

    /// Скормить решения VAD по фреймам, появившимся ПОСЛЕ прошлого вызова.
    pub fn observe(&mut self, voiced: &[bool]) {
        for &v in voiced {
            self.frames_seen += 1;
            if v {
                self.silence_run = 0;
            } else {
                self.silence_run += 1;
                // Граница ставится там, где тишина НАЧАЛАСЬ, а не там, где мы в ней
                // убедились: иначе в зафиксированный отрезок уехала бы пауза целиком,
                // а следующее окно начиналось бы с обрезанного вдоха.
                if self.silence_run >= self.silence_frames_to_close {
                    // Граница записывается БЕЗУСЛОВНО, а годна ли она — решает `plan`.
                    // Здесь стояли ещё два условия (флаг «была ли речь» и
                    // `boundary > committed_samples`), и мутационная проверка показала,
                    // что ни одно из них не ловится тестами: три условия защищали одно
                    // и то же свойство и взаимно маскировали поломку друг друга. Такое
                    // «покрытие» не доказывает ничего, поэтому проверка оставлена ОДНА —
                    // в `plan`, где она наблюдаема снаружи и мутацией краснеет.
                    self.pending_boundary =
                        Some((self.frames_seen - self.silence_run) * self.frame_samples);
                }
            }
        }
    }

    /// Какой отрезок буфера длиной `total_samples` отдавать движку сейчас.
    pub fn plan(&self, total_samples: usize) -> TranscribePlan {
        let start = self.committed_samples.min(total_samples);
        match self.pending_boundary {
            // Граница может оказаться дальше, чем реально доехало сэмплов: решения VAD
            // и буфер читаются из разных мест и не обязаны быть согласованы по длине.
            // Тогда закрывать нечего — ждём, пока буфер догонит.
            Some(b) if b <= total_samples && b > start => TranscribePlan {
                start,
                end: b,
                closes_utterance: true,
            },
            _ => TranscribePlan {
                start,
                end: total_samples.max(start),
                closes_utterance: false,
            },
        }
    }

    /// Зафиксировать текст закрытой реплики и сдвинуть окно. Возвращает полный текст.
    ///
    /// Вызывать ТОЛЬКО для плана с `closes_utterance == true` — иначе окно уехало бы
    /// за неподтверждённый хвост и часть речи потерялась бы навсегда.
    pub fn commit(&mut self, text: &str) -> &str {
        if let Some(b) = self.pending_boundary.take() {
            self.committed_samples = b;
        }
        let t = text.trim();
        if !t.is_empty() {
            if !self.committed_text.is_empty() {
                self.committed_text.push(' ');
            }
            self.committed_text.push_str(t);
        }
        &self.committed_text
    }

    /// Показать зафиксированное вместе с ещё «сырым» хвостом — ничего не меняя.
    pub fn preview(&self, fresh: &str) -> String {
        let f = fresh.trim();
        match (self.committed_text.is_empty(), f.is_empty()) {
            (true, _) => f.to_string(),
            (false, true) => self.committed_text.clone(),
            (false, false) => format!("{} {}", self.committed_text, f),
        }
    }

    /// Уже зафиксированная часть (без хвоста).
    pub fn committed_text(&self) -> &str {
        &self.committed_text
    }

    /// Граница зафиксированного в сэмплах — то, с чего начнётся следующее окно.
    pub fn committed_samples(&self) -> usize {
        self.committed_samples
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const F: usize = 160; // 10 мс при 16 кГц
    const CLOSE: usize = 3;

    fn win() -> SubtitleWindow {
        SubtitleWindow::new(F, CLOSE)
    }

    fn frames(pattern: &str) -> Vec<bool> {
        pattern.chars().map(|c| c == 'v').collect()
    }

    #[test]
    fn without_speech_the_window_never_closes_and_never_commits_emptiness() {
        // Ведущая тишина — самый частый старт диктовки. Если бы она порождала границу,
        // окно коммитило бы пустые строки на каждом тике и текст «моргал» бы.
        let mut w = win();
        w.observe(&frames("ssssssssss"));
        let p = w.plan(10 * F);
        assert!(!p.closes_utterance);
        assert_eq!(p.start, 0);
        assert_eq!(p.end, 10 * F);
        assert_eq!(w.committed_samples(), 0);
    }

    #[test]
    fn short_pause_inside_a_phrase_does_not_close_it() {
        // Двух тихих фреймов при пороге в три недостаточно: иначе окно резалось бы
        // на вдохах и склеенный текст сыпался бы на полуслове.
        let mut w = win();
        w.observe(&frames("vvvssvvv"));
        assert!(!w.plan(8 * F).closes_utterance);
    }

    #[test]
    fn sustained_silence_closes_the_utterance_where_silence_began() {
        // Граница — начало тишины, а не момент, когда мы в ней убедились. Иначе пауза
        // целиком уезжала бы в зафиксированный отрезок, а новое окно начиналось бы
        // с обрезанного начала следующей фразы.
        let mut w = win();
        w.observe(&frames("vvvvsss")); // 4 речи, затем 3 тишины
        let p = w.plan(7 * F);
        assert!(p.closes_utterance);
        assert_eq!(p.start, 0);
        assert_eq!(
            p.end,
            4 * F,
            "граница обязана стоять на 4-м фрейме, где смолкли"
        );
    }

    #[test]
    fn commit_moves_the_window_and_the_next_plan_starts_there() {
        // Ради этого всё и затевалось: следующий тик не должен видеть уже расшифрованное.
        let mut w = win();
        w.observe(&frames("vvvvsss"));
        let p = w.plan(7 * F);
        assert!(p.closes_utterance);
        w.commit("привет мир");

        assert_eq!(w.committed_samples(), 4 * F);
        w.observe(&frames("vv"));
        let p2 = w.plan(9 * F);
        assert_eq!(
            p2.start,
            4 * F,
            "окно обязано начинаться с границы, а не с нуля"
        );
        assert_eq!(p2.end, 9 * F);
        assert!(!p2.closes_utterance);
    }

    #[test]
    fn text_accumulates_across_utterances_and_preview_does_not_mutate_state() {
        let mut w = win();
        w.observe(&frames("vvvvsss"));
        w.commit("первая фраза");
        assert_eq!(w.preview("второ"), "первая фраза второ");
        assert_eq!(
            w.committed_text(),
            "первая фраза",
            "preview обязан быть чистым — он рисует, а не фиксирует"
        );

        w.observe(&frames("vvvvsss"));
        assert!(w.plan(14 * F).closes_utterance);
        w.commit("вторая фраза");
        assert_eq!(w.committed_text(), "первая фраза вторая фраза");
    }

    #[test]
    fn a_silent_utterance_commits_no_stray_whitespace() {
        // Движок на паузе законно возвращает пустую строку. Она не должна оставлять
        // висячий пробел, который потом склеится с началом следующей фразы.
        let mut w = win();
        w.observe(&frames("vvvvsss"));
        w.commit("фраза");
        w.observe(&frames("vsss"));
        assert!(w.plan(18 * F).closes_utterance);
        w.commit("   ");
        assert_eq!(w.committed_text(), "фраза");
        assert_eq!(w.preview(""), "фраза");
    }

    #[test]
    fn boundary_beyond_the_buffer_waits_instead_of_cutting() {
        // Решения VAD и сам буфер читаются из разных мест и не обязаны быть
        // согласованы по длине. Граница за концом доехавших сэмплов — не повод
        // резать: end никогда не должен уходить за total_samples.
        let mut w = win();
        w.observe(&frames("vvvvsss"));
        let p = w.plan(2 * F); // буфер отстал
        assert!(!p.closes_utterance);
        assert_eq!(p.end, 2 * F);
        assert!(p.end >= p.start);
    }

    #[test]
    fn empty_plan_is_recognisable_so_the_engine_is_not_called_for_nothing() {
        let mut w = win();
        w.observe(&frames("vvvvsss"));
        w.commit("фраза");
        // Новых сэмплов после границы ещё нет.
        let p = w.plan(4 * F);
        assert!(
            p.is_empty(),
            "пустой отрезок обязан опознаваться, а не звать движок"
        );
    }

    #[test]
    fn degenerate_parameters_are_clamped_not_silently_accepted() {
        let mut w = SubtitleWindow::new(0, 0);
        w.observe(&frames("vs"));
        // frame_samples=0 сделал бы любую границу нулевой, порог 0 закрывал бы
        // реплику на первой же тишине внутри фразы.
        let p = w.plan(100);
        assert!(p.end >= p.start);
        assert!(w.committed_samples() <= 100);
    }
}
