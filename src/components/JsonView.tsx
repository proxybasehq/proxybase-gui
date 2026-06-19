interface JsonViewProps {
  data: unknown;
  label?: string;
}

export default function JsonView({ data, label }: JsonViewProps) {
  const json = JSON.stringify(data, null, 2);

  return (
    <div>
      {label && <div className="form-label">{label}</div>}
      <pre className="json-view"><code>{json}</code></pre>
    </div>
  );
}
