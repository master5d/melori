import type { ClientNote } from "@/bindings";
import { diagnosisWarning } from "./meetingTools";
import { useTranslation } from "react-i18next";

export function noteRows(note: ClientNote): Array<[string, string]> {
  return note.sections.map(([key, title]) => [title, note.fields[key] ?? ""]);
}

export function noteCopyText(note: ClientNote): string {
  return noteRows(note)
    .map(([title, text]) => `${title}:\n${text}`)
    .join("\n\n");
}

export default function NoteView({ note }: { note: ClientNote }) {
  const { t } = useTranslation();
  const warnings =
    (note as ClientNote & { warnings?: { section: string; phrase: string }[] })
      .warnings ?? [];
  return (
    <div className="mc-note-view">
      {noteRows(note).map(([title, text]) => (
        <section className="mc-sec" key={title}>
          <div className="mc-sec-title">{title}</div>
          <div className="mc-sec-body">{text}</div>
          {warnings
            .filter(
              (warning) =>
                warning.section ===
                note.sections.find(([, name]) => name === title)?.[0],
            )
            .map((warning) => (
              <div
                className="mc-sec-warning"
                key={`${warning.section}-${warning.phrase}`}
              >
                {diagnosisWarning(warning.phrase, t)}
              </div>
            ))}
        </section>
      ))}
    </div>
  );
}
