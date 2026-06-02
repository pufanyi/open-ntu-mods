import ReactMarkdown from "react-markdown";
import rehypeSanitize from "rehype-sanitize";

type MarkdownViewProps = {
  markdown: string;
};

export function MarkdownView({ markdown }: MarkdownViewProps) {
  return (
    <div className="markdown">
      <ReactMarkdown rehypePlugins={[rehypeSanitize]}>{markdown}</ReactMarkdown>
    </div>
  );
}
