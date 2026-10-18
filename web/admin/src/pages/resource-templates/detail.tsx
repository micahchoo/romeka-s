import type { ReactElement } from 'react';
import { useState } from 'react';
import { api } from '../../api/client';
import type {
  PropertyId,
  ResourceTemplate,
  ResourceTemplateProperty,
  TemplateId,
} from '../../api/types';
import { DeleteControl } from '../../app/delete';
import { ErrorNotice } from '../../app/errors';
import { EditPanel } from '../../app/fields';
import {
  classLabelOf,
  classLabels,
  ownerNameOf,
  propertyLabelOf,
  propertyLabels,
  userNames,
} from '../../app/labels';
import { useAsync } from '../../app/useAsync';
import { useTranslator } from '../../i18n';
import { ResourceTemplateEditForm } from './form';

/**
 * One browse page of label references. The template carries its entries as
 * property references, so their labels take a read of the whole property list.
 */
const REFERENCES_PER_PAGE = Number.MAX_SAFE_INTEGER;

interface TemplatePropertyRow {
  readonly propertyId: PropertyId;
  readonly label: string | null;
  readonly dataTypes: readonly string[];
  readonly alternateLabel: string | null;
  readonly alternateComment: string | null;
  readonly isRequired: boolean;
  readonly isPrivate: boolean;
  readonly defaultLang: string | null;
}

interface Loaded {
  readonly template: ResourceTemplate;
  readonly classLabel: string | null;
  readonly ownerName: string | null;
  readonly titlePropertyLabel: string | null;
  readonly descriptionPropertyLabel: string | null;
  readonly rows: readonly TemplatePropertyRow[];
}

const load = async (templateId: TemplateId): Promise<Loaded> => {
  const [template, properties, users, classes] = await Promise.all([
    api.getResourceTemplate(templateId),
    api.listProperties({ perPage: REFERENCES_PER_PAGE, sortBy: 'id', sortOrder: 'asc' }),
    api.listUsers({ perPage: REFERENCES_PER_PAGE }),
    api.listResourceClasses({ perPage: REFERENCES_PER_PAGE, sortBy: 'id', sortOrder: 'asc' }),
  ]);
  const names = propertyLabels(properties.items);
  const row = (entry: ResourceTemplateProperty): TemplatePropertyRow => ({
    propertyId: entry.property.id,
    label: propertyLabelOf(names, entry.property),
    dataTypes: entry.dataTypes,
    alternateLabel: entry.alternateLabel,
    alternateComment: entry.alternateComment,
    isRequired: entry.isRequired,
    isPrivate: entry.isPrivate,
    defaultLang: entry.defaultLang,
  });
  return {
    template,
    classLabel: classLabelOf(classLabels(classes.items), template.resourceClass),
    ownerName: ownerNameOf(userNames(users.items), template.owner),
    titlePropertyLabel: propertyLabelOf(names, template.titleProperty),
    descriptionPropertyLabel: propertyLabelOf(names, template.descriptionProperty),
    rows: template.properties.map(row),
  };
};

/**
 * `Admin/ResourceTemplateController::showAction`: one template, its class, its
 * title and description properties, and a row per template property.
 */
export const ResourceTemplateDetail = ({
  templateId,
}: {
  readonly templateId: TemplateId;
}): ReactElement => {
  const { t } = useTranslator();
  const state = useAsync(() => load(templateId), [templateId]);
  const [edited, setEdited] = useState<ResourceTemplate | null>(null);

  if (state.status === 'loading') {
    return <p role="status">{t('loading.resourceTemplate')}</p>;
  }
  if (state.status === 'error') {
    return <ErrorNotice error={state.error} />;
  }

  const { classLabel, ownerName, titlePropertyLabel, descriptionPropertyLabel, rows } =
    state.data;
  const template = edited ?? state.data.template;
  return (
    <section>
      <h2>{template.label}</h2>

      <dl>
        <dt>{t('column.id')}</dt>
        <dd>{template.id}</dd>
        <dt>{t('column.owner')}</dt>
        <dd>
          {template.owner === null ? (
            t('common.noOwner')
          ) : (
            <a href={`#/users/${String(template.owner.id)}`}>{ownerName ?? t('common.noOwner')}</a>
          )}
        </dd>
        <dt>{t('column.class')}</dt>
        <dd>
          {template.resourceClass === null ? (
            t('common.none')
          ) : (
            <a href={`#/resource-classes/${String(template.resourceClass.id)}`}>
              {classLabel ?? t('common.none')}
            </a>
          )}
        </dd>
        <dt>{t('detail.titleProperty')}</dt>
        <dd>
          {template.titleProperty === null ? (
            t('common.none')
          ) : (
            <a href={`#/properties/${String(template.titleProperty.id)}`}>
              {titlePropertyLabel ?? t('common.none')}
            </a>
          )}
        </dd>
        <dt>{t('detail.descriptionProperty')}</dt>
        <dd>
          {template.descriptionProperty === null ? (
            t('common.none')
          ) : (
            <a href={`#/properties/${String(template.descriptionProperty.id)}`}>
              {descriptionPropertyLabel ?? t('common.none')}
            </a>
          )}
        </dd>
      </dl>

      <h3>{t('column.properties')}</h3>
      <div className="table-scroll">
        <table>
          <thead>
            <tr>
              <th>{t('field.property')}</th>
              <th>{t('detail.dataTypes')}</th>
              <th>{t('detail.alternateLabel')}</th>
              <th>{t('detail.alternateComment')}</th>
              <th>{t('detail.required')}</th>
              <th>{t('common.private')}</th>
              <th>{t('detail.defaultLang')}</th>
            </tr>
          </thead>
          <tbody>
            {rows.length === 0 ? (
              <tr>
                <td colSpan={7}>{t('list.empty', { resource: t('resource.properties') })}</td>
              </tr>
            ) : (
              rows.map((row) => (
                <tr key={String(row.propertyId)}>
                  <td>
                    <a href={`#/properties/${String(row.propertyId)}`}>
                      {row.label ?? t('common.unknown')}
                    </a>
                  </td>
                  <td>{row.dataTypes.join(', ')}</td>
                  <td>{row.alternateLabel ?? t('common.none')}</td>
                  <td>{row.alternateComment ?? t('common.none')}</td>
                  <td>{row.isRequired ? t('common.yes') : t('common.no')}</td>
                  <td>{row.isPrivate ? t('common.yes') : t('common.no')}</td>
                  <td>{row.defaultLang ?? t('common.none')}</td>
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>

      <EditPanel label={t('resourceTemplates.editLabel')}>
        <ResourceTemplateEditForm template={template} onSaved={setEdited} />
      </EditPanel>

      <DeleteControl
        label={t('resourceTemplates.deleteLabel')}
        rowName={template.label}
        onConfirm={async () => {
          await api.deleteResourceTemplate(template.id);
          window.location.hash = '#/resource-templates';
        }}
      />
    </section>
  );
};
