import { AddRounded, DeleteOutlineRounded } from '@mui/icons-material'
import {
  Box,
  IconButton,
  InputAdornment,
  List,
  ListItem,
  ListItemText,
  TextField,
  Tooltip,
} from '@mui/material'
import { useLockFn } from 'ahooks'
import { forwardRef, useImperativeHandle, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { BaseDialog, DialogRef, Switch, TooltipIcon } from '@/components/base'
import { useVerge } from '@/hooks/use-verge'
import { showNotice } from '@/services/notice-service'

// Mirrors IVerge::template() so the form shows the same values the backend
// applies before the user has ever saved this dialog.
const DEFAULT_ENABLE = true
const DEFAULT_GROUP_NAME = '🚀节点选择'
const DEFAULT_TARGET_NAME = '🚀自动优选'
const DEFAULT_TEST_URL = 'http://connectivitycheck.gstatic.com/generate_204'
const DEFAULT_INTERVAL = 60
const DEFAULT_TOLERANCE = 50
const DEFAULT_EXCLUDE_FILTER = '官网网址'
const DEFAULT_PROVIDER_INTERVAL = 86400

interface ExtraProvider {
  uid: string
  name: string
  url: string
  prefix: string
  interval: number
}

let uidSeed = 0
const nextUid = () => `auto-select-provider-${uidSeed++}`

const createProvider = (): ExtraProvider => ({
  uid: nextUid(),
  name: '',
  url: '',
  prefix: '',
  interval: DEFAULT_PROVIDER_INTERVAL,
})

const fallbackNumber = (
  value: number,
  defaultValue: number,
  min: number = 1,
) => (Number.isFinite(value) && value >= min ? value : defaultValue)

export const AutoSelectViewer = forwardRef<DialogRef>((props, ref) => {
  const { t } = useTranslation()
  const { verge, patchVerge } = useVerge()

  const [open, setOpen] = useState(false)
  const [values, setValues] = useState({
    enableAutoSelect: DEFAULT_ENABLE,
    autoSelectGroupName: DEFAULT_GROUP_NAME,
    autoSelectTargetName: DEFAULT_TARGET_NAME,
    autoSelectTestUrl: DEFAULT_TEST_URL,
    autoSelectInterval: DEFAULT_INTERVAL,
    autoSelectTolerance: DEFAULT_TOLERANCE,
    autoSelectExcludeFilter: DEFAULT_EXCLUDE_FILTER,
    autoSelectNamePrefix: '',
    extraProviders: [] as ExtraProvider[],
  })

  useImperativeHandle(ref, () => ({
    open: () => {
      setOpen(true)
      setValues({
        enableAutoSelect: verge?.enable_auto_select ?? DEFAULT_ENABLE,
        autoSelectGroupName:
          verge?.auto_select_group_name ?? DEFAULT_GROUP_NAME,
        autoSelectTargetName:
          verge?.auto_select_target_name ?? DEFAULT_TARGET_NAME,
        autoSelectTestUrl: verge?.auto_select_test_url ?? DEFAULT_TEST_URL,
        autoSelectInterval: verge?.auto_select_interval ?? DEFAULT_INTERVAL,
        autoSelectTolerance: verge?.auto_select_tolerance ?? DEFAULT_TOLERANCE,
        autoSelectExcludeFilter:
          verge?.auto_select_exclude_filter ?? DEFAULT_EXCLUDE_FILTER,
        autoSelectNamePrefix: verge?.auto_select_name_prefix ?? '',
        extraProviders: (verge?.auto_select_extra_providers ?? []).map((p) => ({
          uid: nextUid(),
          name: p.name ?? '',
          url: p.url ?? '',
          prefix: p.prefix ?? '',
          interval: p.interval ?? DEFAULT_PROVIDER_INTERVAL,
        })),
      })
    },
    close: () => setOpen(false),
  }))

  const updateProvider = <K extends keyof ExtraProvider>(
    index: number,
    key: K,
    value: ExtraProvider[K],
  ) => {
    setValues((v) => ({
      ...v,
      extraProviders: v.extraProviders.map((p, i) =>
        i === index ? { ...p, [key]: value } : p,
      ),
    }))
  }

  const removeProvider = (index: number) => {
    setValues((v) => ({
      ...v,
      extraProviders: v.extraProviders.filter((_, i) => i !== index),
    }))
  }

  const onSave = useLockFn(async () => {
    try {
      await patchVerge({
        enable_auto_select: values.enableAutoSelect,
        auto_select_group_name: values.autoSelectGroupName.trim(),
        auto_select_target_name: values.autoSelectTargetName.trim(),
        auto_select_test_url: values.autoSelectTestUrl.trim(),
        auto_select_interval: fallbackNumber(
          values.autoSelectInterval,
          DEFAULT_INTERVAL,
        ),
        auto_select_tolerance: fallbackNumber(
          values.autoSelectTolerance,
          DEFAULT_TOLERANCE,
          0,
        ),
        auto_select_exclude_filter: values.autoSelectExcludeFilter.trim(),
        auto_select_name_prefix: values.autoSelectNamePrefix.trim(),
        auto_select_extra_providers: values.extraProviders
          .filter((p) => p.name.trim() || p.url.trim())
          .map((p) => ({
            name: p.name.trim(),
            url: p.url.trim(),
            prefix: p.prefix.trim(),
            interval: fallbackNumber(p.interval, DEFAULT_PROVIDER_INTERVAL),
          })),
      })
      setOpen(false)
    } catch (err) {
      showNotice.error(err)
    }
  })

  return (
    <BaseDialog
      open={open}
      title={t('settings.modals.autoSelect.title')}
      contentSx={{ width: 620 }}
      okBtn={t('shared.actions.save')}
      cancelBtn={t('shared.actions.cancel')}
      onClose={() => setOpen(false)}
      onCancel={() => setOpen(false)}
      onOk={onSave}
    >
      <List>
        <ListItem sx={{ padding: '5px 2px' }}>
          <ListItemText
            primary={t('settings.modals.autoSelect.fields.enable')}
            sx={{ maxWidth: 'fit-content' }}
          />
          <TooltipIcon
            title={t('settings.modals.autoSelect.tooltips.enable')}
            sx={{ opacity: '0.7' }}
          />
          <Switch
            edge="end"
            checked={values.enableAutoSelect}
            onChange={(_, c) =>
              setValues((v) => ({ ...v, enableAutoSelect: c }))
            }
            sx={{ marginLeft: 'auto' }}
          />
        </ListItem>

        <ListItem sx={{ padding: '5px 2px' }}>
          <ListItemText
            primary={t('settings.modals.autoSelect.fields.groupName')}
            sx={{ maxWidth: 'fit-content' }}
          />
          <TooltipIcon
            title={t('settings.modals.autoSelect.tooltips.groupName')}
            sx={{ opacity: '0.7' }}
          />
          <TextField
            autoComplete="new-password"
            size="small"
            autoCorrect="off"
            autoCapitalize="off"
            spellCheck="false"
            sx={{ width: 320, marginLeft: 'auto' }}
            value={values.autoSelectGroupName}
            disabled={!values.enableAutoSelect}
            onChange={(e) =>
              setValues((v) => ({ ...v, autoSelectGroupName: e.target.value }))
            }
          />
        </ListItem>

        <ListItem sx={{ padding: '5px 2px' }}>
          <ListItemText
            primary={t('settings.modals.autoSelect.fields.targetName')}
            sx={{ maxWidth: 'fit-content' }}
          />
          <TooltipIcon
            title={t('settings.modals.autoSelect.tooltips.targetName')}
            sx={{ opacity: '0.7' }}
          />
          <TextField
            autoComplete="new-password"
            size="small"
            autoCorrect="off"
            autoCapitalize="off"
            spellCheck="false"
            sx={{ width: 320, marginLeft: 'auto' }}
            value={values.autoSelectTargetName}
            disabled={!values.enableAutoSelect}
            onChange={(e) =>
              setValues((v) => ({ ...v, autoSelectTargetName: e.target.value }))
            }
          />
        </ListItem>

        <ListItem sx={{ padding: '5px 2px' }}>
          <ListItemText
            primary={t('settings.modals.autoSelect.fields.testUrl')}
            sx={{ maxWidth: 'fit-content' }}
          />
          <TooltipIcon
            title={t('settings.modals.autoSelect.tooltips.testUrl')}
            sx={{ opacity: '0.7' }}
          />
          <TextField
            autoComplete="new-password"
            size="small"
            autoCorrect="off"
            autoCapitalize="off"
            spellCheck="false"
            sx={{ width: 320, marginLeft: 'auto' }}
            value={values.autoSelectTestUrl}
            disabled={!values.enableAutoSelect}
            placeholder={DEFAULT_TEST_URL}
            onChange={(e) =>
              setValues((v) => ({ ...v, autoSelectTestUrl: e.target.value }))
            }
          />
        </ListItem>

        <ListItem sx={{ padding: '5px 2px' }}>
          <ListItemText
            primary={t('settings.modals.autoSelect.fields.interval')}
            sx={{ maxWidth: 'fit-content' }}
          />
          <TooltipIcon
            title={t('settings.modals.autoSelect.tooltips.interval')}
            sx={{ opacity: '0.7' }}
          />
          <TextField
            autoComplete="new-password"
            size="small"
            type="number"
            autoCorrect="off"
            autoCapitalize="off"
            spellCheck="false"
            sx={{ width: 320, marginLeft: 'auto' }}
            value={values.autoSelectInterval}
            disabled={!values.enableAutoSelect}
            onChange={(e) => {
              const parsed = parseInt(e.target.value, 10)
              setValues((v) => ({
                ...v,
                autoSelectInterval: Number.isFinite(parsed) ? parsed : 0,
              }))
            }}
            slotProps={{
              input: {
                endAdornment: (
                  <InputAdornment position="end">
                    {t('shared.units.seconds')}
                  </InputAdornment>
                ),
              },
            }}
          />
        </ListItem>

        <ListItem sx={{ padding: '5px 2px' }}>
          <ListItemText
            primary={t('settings.modals.autoSelect.fields.tolerance')}
            sx={{ maxWidth: 'fit-content' }}
          />
          <TooltipIcon
            title={t('settings.modals.autoSelect.tooltips.tolerance')}
            sx={{ opacity: '0.7' }}
          />
          <TextField
            autoComplete="new-password"
            size="small"
            type="number"
            autoCorrect="off"
            autoCapitalize="off"
            spellCheck="false"
            sx={{ width: 320, marginLeft: 'auto' }}
            value={values.autoSelectTolerance}
            disabled={!values.enableAutoSelect}
            onChange={(e) => {
              const parsed = parseInt(e.target.value, 10)
              setValues((v) => ({
                ...v,
                autoSelectTolerance: Number.isFinite(parsed) ? parsed : 0,
              }))
            }}
            slotProps={{
              input: {
                endAdornment: (
                  <InputAdornment position="end">
                    {t('shared.units.milliseconds')}
                  </InputAdornment>
                ),
              },
            }}
          />
        </ListItem>

        <ListItem sx={{ padding: '5px 2px' }}>
          <ListItemText
            primary={t('settings.modals.autoSelect.fields.excludeFilter')}
            sx={{ maxWidth: 'fit-content' }}
          />
          <TooltipIcon
            title={t('settings.modals.autoSelect.tooltips.excludeFilter')}
            sx={{ opacity: '0.7' }}
          />
          <TextField
            autoComplete="new-password"
            size="small"
            autoCorrect="off"
            autoCapitalize="off"
            spellCheck="false"
            sx={{ width: 320, marginLeft: 'auto' }}
            value={values.autoSelectExcludeFilter}
            disabled={!values.enableAutoSelect}
            onChange={(e) =>
              setValues((v) => ({
                ...v,
                autoSelectExcludeFilter: e.target.value,
              }))
            }
          />
        </ListItem>

        <ListItem sx={{ padding: '5px 2px' }}>
          <ListItemText
            primary={t('settings.modals.autoSelect.fields.namePrefix')}
            sx={{ maxWidth: 'fit-content' }}
          />
          <TooltipIcon
            title={t('settings.modals.autoSelect.tooltips.namePrefix')}
            sx={{ opacity: '0.7' }}
          />
          <TextField
            autoComplete="new-password"
            size="small"
            autoCorrect="off"
            autoCapitalize="off"
            spellCheck="false"
            sx={{ width: 320, marginLeft: 'auto' }}
            value={values.autoSelectNamePrefix}
            disabled={!values.enableAutoSelect}
            onChange={(e) =>
              setValues((v) => ({ ...v, autoSelectNamePrefix: e.target.value }))
            }
          />
        </ListItem>

        <ListItem sx={{ padding: '5px 2px' }}>
          <ListItemText
            primary={t('settings.modals.autoSelect.fields.extraProviders')}
            sx={{ maxWidth: 'fit-content' }}
          />
          <TooltipIcon
            title={t('settings.modals.autoSelect.tooltips.extraProviders')}
            sx={{ opacity: '0.7' }}
          />
          <Tooltip title={t('settings.modals.autoSelect.extraProviders.add')}>
            <IconButton
              size="small"
              sx={{ marginLeft: 'auto' }}
              disabled={!values.enableAutoSelect}
              onClick={() =>
                setValues((v) => ({
                  ...v,
                  extraProviders: [...v.extraProviders, createProvider()],
                }))
              }
            >
              <AddRounded fontSize="small" />
            </IconButton>
          </Tooltip>
        </ListItem>

        {values.extraProviders.length === 0 ? (
          <ListItem sx={{ padding: '5px 2px' }}>
            <ListItemText
              secondary={t('settings.modals.autoSelect.extraProviders.empty')}
            />
          </ListItem>
        ) : (
          values.extraProviders.map((provider, index) => (
            <ListItem
              key={provider.uid}
              sx={{ padding: '5px 2px', gap: 1, alignItems: 'center' }}
            >
              <Box sx={{ display: 'flex', gap: 1, flex: 1 }}>
                <TextField
                  autoComplete="new-password"
                  size="small"
                  autoCorrect="off"
                  autoCapitalize="off"
                  spellCheck="false"
                  sx={{ width: 120 }}
                  placeholder={t(
                    'settings.modals.autoSelect.extraProviders.name',
                  )}
                  value={provider.name}
                  disabled={!values.enableAutoSelect}
                  onChange={(e) =>
                    updateProvider(index, 'name', e.target.value)
                  }
                />
                <TextField
                  autoComplete="new-password"
                  size="small"
                  autoCorrect="off"
                  autoCapitalize="off"
                  spellCheck="false"
                  sx={{ flex: 1 }}
                  placeholder={t(
                    'settings.modals.autoSelect.extraProviders.url',
                  )}
                  value={provider.url}
                  disabled={!values.enableAutoSelect}
                  onChange={(e) => updateProvider(index, 'url', e.target.value)}
                />
                <TextField
                  autoComplete="new-password"
                  size="small"
                  autoCorrect="off"
                  autoCapitalize="off"
                  spellCheck="false"
                  sx={{ width: 100 }}
                  placeholder={t(
                    'settings.modals.autoSelect.extraProviders.prefix',
                  )}
                  value={provider.prefix}
                  disabled={!values.enableAutoSelect}
                  onChange={(e) =>
                    updateProvider(index, 'prefix', e.target.value)
                  }
                />
                <TextField
                  autoComplete="new-password"
                  size="small"
                  type="number"
                  autoCorrect="off"
                  autoCapitalize="off"
                  spellCheck="false"
                  sx={{ width: 120 }}
                  placeholder={t(
                    'settings.modals.autoSelect.extraProviders.interval',
                  )}
                  value={provider.interval}
                  disabled={!values.enableAutoSelect}
                  onChange={(e) => {
                    const parsed = parseInt(e.target.value, 10)
                    updateProvider(
                      index,
                      'interval',
                      Number.isFinite(parsed) ? parsed : 0,
                    )
                  }}
                />
              </Box>
              <Tooltip
                title={t('settings.modals.autoSelect.extraProviders.remove')}
              >
                <IconButton
                  size="small"
                  disabled={!values.enableAutoSelect}
                  onClick={() => removeProvider(index)}
                >
                  <DeleteOutlineRounded fontSize="small" />
                </IconButton>
              </Tooltip>
            </ListItem>
          ))
        )}
      </List>
    </BaseDialog>
  )
})
