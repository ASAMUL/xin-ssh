import { LanguageDescription, LanguageSupport, StreamLanguage } from "@codemirror/language";
import { languages } from "@codemirror/language-data";

// legacy-modes 的 nginx 模式是从 CSS 模式改来的，留着 CSS 的 /* */ 块注释。
// nginx 只有 # 行注释，可 include conf.d/*.conf 这种通配路径里恰好有 /*，
// 被当成块注释开头后一直找不到 */，整份文件剩下的部分全被染成注释色
function withoutBlockComments(parser) {
  const baseTokenize = parser.startState().tokenize;

  return {
    ...parser,
    token(stream, state) {
      const style = parser.token(stream, state);
      if (style !== "comment") return style;

      const text = stream.current();
      const blockStart = text.indexOf("/*");
      const hashStart = text.indexOf("#");
      if (blockStart < 0 || (hashStart >= 0 && hashStart < blockStart)) return style;

      // 只吃掉 /，剩下的从 * 开始按普通内容重新切
      stream.backUp(text.length - blockStart - 1);
      state.tokenize = baseTokenize;
      return null;
    },
  };
}

const nginxLanguage = LanguageDescription.of({
  name: "Nginx",
  filename: /nginx.*\.conf$/i,
  load: () =>
    import("@codemirror/legacy-modes/mode/nginx").then(
      ({ nginx }) => new LanguageSupport(StreamLanguage.define(withoutBlockComments(nginx))),
    ),
});

export const editorLanguages = languages.map((description) =>
  description.name === nginxLanguage.name ? nginxLanguage : description,
);
