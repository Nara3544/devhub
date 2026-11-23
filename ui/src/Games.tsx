import { useCallback, useEffect, useState } from 'react'
import { Button } from '@astryxdesign/core/Button'
import { Card } from '@astryxdesign/core/Card'
import { Heading } from '@astryxdesign/core/Heading'
import { Text } from '@astryxdesign/core/Text'
import { VStack } from '@astryxdesign/core/VStack'
import { HStack } from '@astryxdesign/core/HStack'
import { Switch } from '@astryxdesign/core/Switch'
import { Selector } from '@astryxdesign/core/Selector'
import { TextInput } from '@astryxdesign/core/TextInput'
import { CheckboxInput } from '@astryxdesign/core/CheckboxInput'
import { StatusDot } from '@astryxdesign/core/StatusDot'
import { api, PROFILES } from './api'

type Rule = {
  name: string; enabled: boolean; exe: string[]; keyboardProfile: number; baseProfile: number
  disableKeys: string[]; allowWithFn: boolean; mouseProfile: number | null
}
type Mode = { active: string | null; foreground: string; lastError: string | null }

const DISABLE_CHOICES = [
  { key: 'Win', label: 'Windows キー' },
  { key: 'Menu', label: 'メニューキー' },
  { key: 'Caps', label: 'Caps Lock' },
  { key: 'かな', label: 'かな' },
]

const PRESET: Rule = {
  name: 'Apex Legends', enabled: true, exe: ['r5apex.exe', 'r5apex_dx12.exe'],
  keyboardProfile: 4, baseProfile: 0, disableKeys: ['Win'], allowWithFn: true, mouseProfile: null,
}

export function GamesPanel({ onToast }: { onToast: (m: string, e?: boolean) => void }) {
  const [rules, setRules] = useState<Rule[]>([])
  const [mode, setMode] = useState<Mode | null>(null)
  const [busy, setBusy] = useState(false)
  const [dirty, setDirty] = useState(false)

  const load = useCallback(async () => {
    const d = await api.get<{ rules: Rule[]; state: Mode }>('/api/games')
    if (!dirty) setRules(d.rules)
    setMode(d.state)
  }, [dirty])
  useEffect(() => {
    load().catch(() => {})
    const t = setInterval(() => load().catch(() => {}), 2000)
    return () => clearInterval(t)
  }, [load])

  const edit = (i: number, r: Partial<Rule>) => { setRules(rules.map((x, j) => (j === i ? { ...x, ...r } : x))); setDirty(true) }

  const save = async (next = rules) => {
    setBusy(true)
    try {
      const d = await api.put<{ rules: Rule[]; state: Mode }>('/api/games', next)
      setRules(d.rules); setMode(d.state); setDirty(false)
      onToast('保存しました（ゲーム用プロファイルも更新）')
    } catch (e) { onToast((e as Error).message, true) } finally { setBusy(false) }
  }

  const profileOptions = PROFILES.map((n, i) => ({ value: String(i), label: n })).slice(1)

  return (
    <VStack gap={4}>
      <Card padding={4}>
        <HStack gap={3} vAlign="center">
          <StatusDot variant={mode?.active ? 'accent' : 'neutral'} label={mode?.active ? 'ゲームモード中' : '通常'} isPulsing={!!mode?.active} />
          <Text>{mode?.active ? `ゲームモード: ${mode.active}` : '通常モード'}</Text>
          <Text color="secondary" size="sm">前面: {mode?.foreground || '-'}</Text>
        </HStack>
        {mode?.lastError && <Text color="secondary" size="sm">エラー: {mode.lastError}</Text>}
      </Card>

      <Text color="secondary" size="sm">
        ゲームが前面にある間だけ、キーボードを専用プロファイルに切り替えます。専用プロファイルはベースのコピーで、選んだキーだけ無効になります。ベースを変えると自動で追従します。
      </Text>

      {rules.map((r, i) => (
        <Card key={i} padding={4}>
          <VStack gap={3}>
            <HStack gap={3} vAlign="center" justify="between">
              <Heading level={3}>{r.name}</Heading>
              <Switch label="有効" value={r.enabled} onChange={v => edit(i, { enabled: v })} />
            </HStack>
            <HStack gap={3} vAlign="end">
              <TextInput label="名前" value={r.name} width={180} onChange={v => edit(i, { name: v })} />
              <TextInput label="実行ファイル（カンマ区切り）" value={r.exe.join(', ')} width={300}
                onChange={v => edit(i, { exe: v.split(',').map(s => s.trim().toLowerCase()).filter(Boolean) })} />
            </HStack>
            <HStack gap={3} vAlign="end">
              <Selector label="ゲーム用プロファイル" width={180} options={profileOptions} value={String(r.keyboardProfile)}
                onChange={v => edit(i, { keyboardProfile: Number(v) })} />
              <Selector label="マウス" width={160}
                options={[{ value: '', label: '切り替えない' }, ...[1, 2, 3].map(n => ({ value: String(n), label: `プロファイル ${n}` }))]}
                value={r.mouseProfile == null ? '' : String(r.mouseProfile)}
                onChange={v => edit(i, { mouseProfile: v ? Number(v) : null })} />
            </HStack>
            <VStack gap={1}>
              <Text weight="semibold" size="sm">ゲーム中に無効にするキー</Text>
              <HStack gap={4}>
                {DISABLE_CHOICES.map(c => (
                  <CheckboxInput key={c.key} label={c.label} value={r.disableKeys.includes(c.key)}
                    onChange={(v: boolean) => edit(i, { disableKeys: v ? [...r.disableKeys, c.key] : r.disableKeys.filter(k => k !== c.key) })} />
                ))}
              </HStack>
              <Switch label="Fn と同時押しなら有効" description="例: Fn + Windows キーで Windows キーが効く" value={r.allowWithFn !== false}
                onChange={v => edit(i, { allowWithFn: v })} />
            </VStack>
            <HStack gap={2}>
              <Button label="削除" size="sm" variant="ghost" onClick={() => { setRules(rules.filter((_, j) => j !== i)); setDirty(true) }} />
            </HStack>
          </VStack>
        </Card>
      ))}

      <HStack gap={2}>
        {!rules.some(r => r.name === PRESET.name) && (
          <Button label="Apex を追加" variant="secondary" onClick={() => { setRules([...rules, PRESET]); setDirty(true) }} />
        )}
        <Button label="ゲームを追加" variant="secondary"
          onClick={() => { setRules([...rules, { ...PRESET, name: '新しいゲーム', exe: [], keyboardProfile: 3 }]); setDirty(true) }} />
        <Button label="保存" variant="primary" isDisabled={!dirty} isLoading={busy} onClick={() => save()} />
      </HStack>
    </VStack>
  )
}
