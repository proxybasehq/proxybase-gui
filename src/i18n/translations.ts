import en from "./lang/en";
import es from "./lang/es";
import fr from "./lang/fr";
import de from "./lang/de";
import it from "./lang/it";
import pt from "./lang/pt";
import ru from "./lang/ru";
import zh from "./lang/zh";
import ja from "./lang/ja";
import ko from "./lang/ko";
import ar from "./lang/ar";
import tr from "./lang/tr";
import pl from "./lang/pl";
import uk from "./lang/uk";
import vi from "./lang/vi";
import id from "./lang/id";

/** All UI message keys, derived from the English dictionary. */
export type Messages = typeof en;

export const translations: Record<string, Messages> = {
  en,
  es,
  fr,
  de,
  it,
  pt,
  ru,
  zh,
  ja,
  ko,
  ar,
  tr,
  pl,
  uk,
  vi,
  id,
};
