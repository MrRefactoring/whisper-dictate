interface InkWaveProps {
  level: number;
  active: boolean;
}

export function InkWave({ level, active }: InkWaveProps) {
  return (
    <div
      className={`inkwave ${active ? "is-active" : ""}`}
      style={{ ["--level" as string]: level.toFixed(3) }}
      aria-hidden="true"
    >
      <span className="inkwave__ring" style={{ ["--d" as string]: "0s" }} />
      <span className="inkwave__ring" style={{ ["--d" as string]: "-0.9s" }} />
      <span className="inkwave__ring" style={{ ["--d" as string]: "-1.8s" }} />
    </div>
  );
}
