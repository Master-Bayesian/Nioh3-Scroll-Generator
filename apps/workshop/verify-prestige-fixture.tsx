import {setUiLocale} from "./presentation";
const locale=new URLSearchParams(location.search).get("locale") as "zh-CN"|"en-US"|"ja-JP"|null;
if(locale)setUiLocale(locale);
void import("./main");
