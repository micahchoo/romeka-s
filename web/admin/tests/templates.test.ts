/**
 * The page template model: the generated built-in registry, the per-site saved
 * row, and the wire shape the site write stores. The saved row is the
 * `romeka_page_templates` site setting, which upstream ignores.
 */
import { describe, expect, it } from 'vitest';
import { BUILTIN_TEMPLATES, parseTemplateRegistry } from '../src/pages/sites/templateRegistry';
import {
  SAVED_TEMPLATES_KEY,
  builtinChoices,
  newTemplateId,
  savedChoices,
  savedTemplates,
  savedTemplatesWire,
} from '../src/pages/sites/templates';

describe('the generated built-in templates', () => {
  it('parses the five starting points', () => {
    expect(BUILTIN_TEMPLATES.map((template) => template.id)).toEqual([
      'exhibit-landing',
      'collection-highlight',
      'about',
      'gallery',
      'blank',
    ]);
  });

  it('rejects a label that is not a catalogue key', () => {
    expect(() =>
      parseTemplateRegistry({
        templates: [
          {
            id: 'x',
            label: 'not.a.key',
            description: 'template.about.description',
            layout: 'normal',
            layoutData: {},
            blocks: [],
          },
        ],
      }),
    ).toThrow(/catalogue key/);
  });

  it('resolves a built-in name through the catalogue', () => {
    const choices = builtinChoices(BUILTIN_TEMPLATES, (key) => key);
    expect(choices[0]).toMatchObject({
      id: 'exhibit-landing',
      name: 'template.exhibitLanding.label',
    });
  });
});

describe('saved templates', () => {
  const stored = {
    id: 't1',
    name: 'Mine',
    description: 'A description',
    layout: 'grid',
    layout_data: { grid_columns: 3 },
    blocks: [{ layout: 'media', data: {}, layout_data: { grid_column_span: '1' } }],
  };
  const settings = { [SAVED_TEMPLATES_KEY]: [stored] };

  it('reads the row the site document carries', () => {
    const templates = savedTemplates(settings);
    expect(templates).toHaveLength(1);
    expect(templates[0]).toMatchObject({
      id: 't1',
      name: 'Mine',
      layout: 'grid',
      layoutData: { grid_columns: 3 },
    });
    expect(templates[0]?.blocks[0]).toEqual({
      layout: 'media',
      data: {},
      layoutData: { grid_column_span: '1' },
    });
  });

  it('returns nothing for a site with no such row', () => {
    expect(savedTemplates(null)).toEqual([]);
    expect(savedTemplates({})).toEqual([]);
    expect(savedTemplates({ [SAVED_TEMPLATES_KEY]: 'nope' })).toEqual([]);
  });

  it('drops an entry that is not a template', () => {
    const templates = savedTemplates({
      [SAVED_TEMPLATES_KEY]: [
        { id: 't1', name: 'Mine', description: '', layout: 'normal', layout_data: {}, blocks: [] },
        { nope: true },
      ],
    });
    expect(templates.map((template) => template.id)).toEqual(['t1']);
  });

  it('writes the row shape the site write stores', () => {
    expect(savedTemplatesWire(savedTemplates(settings))).toEqual([stored]);
  });

  it('shows a saved template as a chooser choice', () => {
    const choices = savedChoices(savedTemplates(settings));
    expect(choices[0]).toMatchObject({ id: 't1', name: 'Mine', description: 'A description' });
  });

  it('mints a new id for each template', () => {
    expect(newTemplateId()).not.toBe(newTemplateId());
  });
});
