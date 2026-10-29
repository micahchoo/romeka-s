/**
 * The `html` field control, through `BlockForm`: typing dispatches the HTML the
 * editing surface holds, each toolbar command wraps the selection, the link
 * command wraps it in an anchor, and the source view edits the raw string.
 */
import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { BlockForm } from '../src/pages/sites/editor/fields/BlockForm';
import {
  actionsOf,
  BlockFormHarness,
  dispatchSpy,
  draftFor,
  schemaForLayout,
  SITE_ID,
  selectAll,
} from './editor-fields-helpers';

/** The html block's own hint (`fieldExample.html.html`). */
const HINT = 'Write what visitors read here.';

const editor = (): HTMLElement => screen.getByRole('textbox', { name: 'Text' });

const renderHtml = (html: string) => {
  const block = draftFor('html', { html });
  const dispatch = dispatchSpy();
  render(
    <BlockForm
      block={block}
      schema={schemaForLayout('html')}
      issues={[]}
      siteId={SITE_ID}
      dispatch={dispatch}
    />,
  );
  return { dispatch, block };
};

const dispatchedHtml = (dispatch: ReturnType<typeof dispatchSpy>): string => {
  const action = actionsOf(dispatch)[0];
  if (action === undefined || action.type !== 'setField' || typeof action.value !== 'string') {
    throw new Error('expected a setField action with a string value');
  }
  return action.value;
};

describe('the html field control', () => {
  it('typing dispatches the editing surface HTML', () => {
    const { dispatch, block } = renderHtml('<p>Hello</p>');
    const surface = editor();
    surface.innerHTML = '<p>Hi there</p>';

    fireEvent.input(surface);

    expect(actionsOf(dispatch)).toEqual([
      { type: 'setField', blockKey: block.key, field: 'html', value: '<p>Hi there</p>' },
    ]);
  });

  it('bold wraps the selection in strong', () => {
    const { dispatch } = renderHtml('<p>Hello</p>');
    const paragraph = editor().querySelector('p');
    if (paragraph === null) {
      throw new Error('expected the stored paragraph');
    }
    selectAll(paragraph);

    fireEvent.click(screen.getByRole('button', { name: 'Bold' }));

    expect(editor().innerHTML).toBe('<p><strong>Hello</strong></p>');
    expect(dispatchedHtml(dispatch)).toBe('<p><strong>Hello</strong></p>');
  });

  it('italic wraps the selection in em', () => {
    const { dispatch } = renderHtml('<p>Hello</p>');
    const paragraph = editor().querySelector('p');
    if (paragraph === null) {
      throw new Error('expected the stored paragraph');
    }
    selectAll(paragraph);

    fireEvent.click(screen.getByRole('button', { name: 'Italic' }));

    expect(dispatchedHtml(dispatch)).toBe('<p><em>Hello</em></p>');
  });

  it('a heading wraps the selection in the heading element', () => {
    const { dispatch } = renderHtml('<p>Hello</p>');
    const paragraph = editor().querySelector('p');
    if (paragraph === null) {
      throw new Error('expected the stored paragraph');
    }
    selectAll(paragraph);

    fireEvent.click(screen.getByRole('button', { name: 'Heading 2' }));

    expect(dispatchedHtml(dispatch)).toContain('<h2>');
  });

  it('a list wraps the selection in a list item', () => {
    const { dispatch } = renderHtml('<p>Hello</p>');
    const paragraph = editor().querySelector('p');
    if (paragraph === null) {
      throw new Error('expected the stored paragraph');
    }
    selectAll(paragraph);

    fireEvent.click(screen.getByRole('button', { name: 'Bulleted list' }));

    expect(dispatchedHtml(dispatch)).toContain('<ul><li>');
  });

  it('the link command wraps the saved selection in an anchor', () => {
    const { dispatch } = renderHtml('<p>Hello</p>');
    const paragraph = editor().querySelector('p');
    if (paragraph === null) {
      throw new Error('expected the stored paragraph');
    }
    selectAll(paragraph);

    fireEvent.click(screen.getByRole('button', { name: 'Link' }));
    fireEvent.change(screen.getByLabelText('URL'), {
      target: { value: 'https://example.org' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Apply link' }));

    expect(dispatchedHtml(dispatch)).toBe(
      '<p><a href="https://example.org">Hello</a></p>',
    );
  });

  it('the source view edits the raw HTML', () => {
    const { dispatch, block } = renderHtml('<p>Hello</p>');

    fireEvent.click(screen.getByRole('button', { name: 'HTML source' }));
    const textarea = editor();
    expect(textarea.tagName).toBe('TEXTAREA');
    fireEvent.change(textarea, { target: { value: '<p>Raw</p>' } });

    expect(actionsOf(dispatch)).toEqual([
      { type: 'setField', blockKey: block.key, field: 'html', value: '<p>Raw</p>' },
    ]);
  });

  it.each(['', '<p><br></p>', '<p>&nbsp;</p>', '<div><br></div>'])(
    'shows its hint while the value %j is blank, as the row\'s "Not written yet" reads it',
    (html) => {
      renderHtml(html);

      expect(screen.getByText(HINT)).toBeInTheDocument();
    },
  );

  it('hides its hint once the text has words, and shows it again when emptied', () => {
    render(<BlockFormHarness layout="html" data={{ html: '' }} />);
    const surface = editor();
    expect(screen.getByText(HINT)).toBeInTheDocument();

    surface.innerHTML = '<p>Hi</p>';
    fireEvent.input(surface);
    expect(screen.queryByText(HINT)).toBeNull();

    surface.innerHTML = '<p><br></p>';
    fireEvent.input(surface);
    expect(screen.getByText(HINT)).toBeInTheDocument();
  });

  it('the source toggle reports its pressed state', () => {
    renderHtml('<p>Hello</p>');
    const toggle = screen.getByRole('button', { name: 'HTML source' });
    expect(toggle).toHaveAttribute('aria-pressed', 'false');

    fireEvent.click(toggle);

    expect(toggle).toHaveAttribute('aria-pressed', 'true');
  });
});
