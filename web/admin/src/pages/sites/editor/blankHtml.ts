/**
 * Whether stored HTML is blank: the one rule the admin reads for it. The
 * Outline row's "Not written yet" (`written.ts`) and the html field's hint
 * (`fields/HtmlEditor.tsx`) both read it, so the hint shows exactly while the
 * row says the text is not written. `blocks.json` carries the server's
 * blank-HTML samples, and a test holds `isBlankHtml` to them.
 */

/** The tags a rich editor leaves in an emptied field. Any other tag is content. */
const EMPTY_TAGS = new Set(['p', 'br', 'div', 'span']);

const isBlankText = (text: string): boolean =>
  text.replaceAll('&nbsp;', ' ').replaceAll('&#160;', ' ').trim() === '';

/**
 * Whether HTML holds no words and no element a visitor would see: only
 * whitespace, non-breaking spaces and the empty wrappers a rich editor leaves
 * (`<p><br></p>`). The same scan as the server's `is_blank_html`.
 */
export const isBlankHtml = (html: string): boolean => {
  let rest = html;
  for (let start = rest.indexOf('<'); start !== -1; start = rest.indexOf('<')) {
    if (!isBlankText(rest.slice(0, start))) {
      return false;
    }
    const end = rest.indexOf('>', start);
    if (end === -1) {
      return false;
    }
    const name = /^\/?([A-Za-z0-9]*)/.exec(rest.slice(start + 1, end))?.[1] ?? '';
    if (!EMPTY_TAGS.has(name.toLowerCase())) {
      return false;
    }
    rest = rest.slice(end + 1);
  }
  return isBlankText(rest);
};
