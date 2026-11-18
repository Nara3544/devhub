import { useCallback, useEffect, useState } from 'react'
import { Button } from '@astryxdesign/core/Button'
import { Card } from '@astryxdesign/core/Card'
import { Heading } from '@astryxdesign/core/Heading'
import { Text } from '@astryxdesign/core/Text'
import { VStack } from '@astryxdesign/core/VStack'
import { HStack } from '@astryxdesign/core/HStack'
import { Slider } from '@astryxdesign/core/Slider'
import { Switch } from '@astryxdesign/core/Switch'
import { Selector } from '@astryxdesign/core/Selector'
import { NumberInput } from '@astryxdesign/core/NumberInput'
import { Banner } from '@astryxdesign/core/Banner'
import { SegmentedControl, SegmentedControlItem } from '@astryxdesign/core/SegmentedControl'
import { api, type MouseData } from './api'

const RATES: Record<string, number[]> = {
  '有線': [125, 250, 500, 1000],
  '1K ドングル': [125, 250, 500, 1000],
  '8K ドングル': [500, 1000, 2000, 4000, 8000],
}
const SLEEP = [
  { value: '10', label: '10秒' }, { value: '30', label: '30秒' }, { value: '60', label: '1分' },
  { value: '300', label: '5分' }, { value: '600', label: '10分' }, { value: '1800', label: '30分' },
]

export function MousePanel({ onToast }: { onToast: (msg: string, err?: boolean) => void }) {
  const [d, setD] = useState<MouseData | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const [stages, setStages] = useState<{ x: number; y: number }[]>([])

  const load = useCallback(async (p?: number) => {
    try {
      const m = await api.get<MouseData>(p == null ? '/api/mouse' : `/api/mouse?profile=${p}`)
      setD(m); setStages(m.dpiStages); setError(null)
    } catch (e) { setError((e as Error).message) }
  }, [])
  useEffect(() => { load() }, [load])

  const set = async (patch: Partial<MouseData>, msg = '保存しました') => {
    if (!d) return
    setBusy(true)
    try {
      const m = await api.patch<MouseData>('/api/mouse', { profile: d.profile, ...patch })
      setD(m); setStages(m.dpiStages)
      onToast(msg)
    } catch (e) { onToast((e as Error).message, true) } finally { setBusy(false) }
  }

  if (error) {
    return <Banner status="warning" title="マウスに接続できません" description={error}
      endContent={<Button label="再試行" variant="secondary" onClick={() => load()} />} />
  }
  if (!d) return <Text color="secondary">読み込み中…</Text>

  const rates = RATES[d.connection] ?? RATES['1K ドングル']
  const stagesChanged = JSON.stringify(stages) !== JSON.stringify(d.dpiStages)

  return (
    <VStack gap={4}>
      <HStack gap={3} vAlign="center" justify="between">
        <SegmentedControl label="プロファイル" value={String(d.profile)} onChange={v => load(Number(v))}>
          {[1, 2, 3].map(i => (
            <SegmentedControlItem key={i} value={String(i)} label={i === d.activeProfile ? `${i} ●` : String(i)} />
          ))}
        </SegmentedControl>
        <HStack gap={2} vAlign="center">
          <Text color="secondary" size="sm">{d.connection}{d.battery != null ? ` / 電池 ${d.battery}%${d.charging ? '（充電中）' : ''}` : ''}</Text>
          {d.profile !== d.activeProfile && (
            <Button label="このプロファイルを使う" variant="secondary" isDisabled={busy} onClick={() => set({ activeProfile: d.profile } as never, '切り替えました')} />
          )}
        </HStack>
      </HStack>

      <div className="cols">
        <Card padding={4}>
          <VStack gap={4}>
            <Heading level={3}>DPI</Heading>
            <div className="stages">
              {stages.map((s, i) => (
                <div className="stage" key={i}>
                  <button type="button" className={`stage-no${d.activeStage === i + 1 ? ' active' : ''}`}
                    title="この段階を使う" onClick={() => set({ activeStage: i + 1 }, `${i + 1}段目に切り替えました`)}>{i + 1}</button>
                  <NumberInput label={d.dpiXy ? 'X' : 'DPI'} isLabelHidden={i > 0} value={s.x} min={50} max={30000} step={50} isIntegerOnly width={120}
                    onChange={v => setStages(stages.map((t, j) => (j === i ? { x: v, y: d.dpiXy ? t.y : v } : t)))} />
                  {d.dpiXy && (
                    <NumberInput label="Y" isLabelHidden={i > 0} value={s.y} min={50} max={30000} step={50} isIntegerOnly width={120}
                      onChange={v => setStages(stages.map((t, j) => (j === i ? { ...t, y: v } : t)))} />
                  )}
                  {stages.length > 1 && <Button label="削除" size="sm" variant="ghost" onClick={() => setStages(stages.filter((_, j) => j !== i))} />}
                </div>
              ))}
            </div>
            <HStack gap={2}>
              {stages.length < 5 && <Button label="段階を追加" size="sm" variant="secondary"
                onClick={() => setStages([...stages, { ...(stages[stages.length - 1] ?? { x: 800, y: 800 }) }])} />}
              <Button label="DPI を保存" size="sm" variant="primary" isDisabled={!stagesChanged} isLoading={busy}
                onClick={() => set({ dpiStages: stages.map(s => ({ x: Math.round(s.x / 50) * 50, y: Math.round(s.y / 50) * 50 })) }, 'DPI を保存しました')} />
            </HStack>
            <Switch label="X / Y 別々に設定" value={d.dpiXy} isDisabled={busy} onChange={v => set({ dpiXy: v })} />

            <SegmentedControl label="ポーリングレート (Hz)" value={String(d.pollingRate)} onChange={v => set({ pollingRate: Number(v) })}>
              {rates.map(r => <SegmentedControlItem key={r} value={String(r)} label={String(r)} />)}
            </SegmentedControl>
            <SegmentedControl label="リフトオフ距離 (LOD)" value={String(d.lod)} onChange={v => set({ lod: Number(v) })}>
              {[0.7, 1, 2].map(l => <SegmentedControlItem key={l} value={String(l)} label={`${l}mm`} />)}
            </SegmentedControl>
          </VStack>
        </Card>

        <Card padding={4}>
          <VStack gap={4}>
            <Heading level={3}>クリック・センサー</Heading>
            <Slider label="デバウンス（チャタリング対策）" description="ダブルクリック化するなら上げる" min={0} max={15} step={1}
              value={d.debounceMs} valueDisplay="text" formatValue={(v: number) => `${v}ms`} isDisabled={busy}
              onChange={(v: number) => setD({ ...d, debounceMs: v })} onChangeEnd={(v: number) => set({ debounceMs: v })} />
            <Switch label="モーションシンク" value={d.motionSync} isDisabled={busy} onChange={v => set({ motionSync: v })} />
            <Switch label="競技モード" description="高速モード。電池消費が増える" value={d.competitionMode} isDisabled={busy} onChange={v => set({ competitionMode: v })} />
            <Switch label="リップルコントロール" value={d.rippleControl} isDisabled={busy} onChange={v => set({ rippleControl: v })} />
            <Switch label="角度スナップ" value={d.angleSnap} isDisabled={busy} onChange={v => set({ angleSnap: v })} />
            <Slider label="角度補正" min={-30} max={30} step={1} value={d.angleTune} valueDisplay="text" formatValue={(v: number) => `${v}°`} isDisabled={busy}
              onChange={(v: number) => setD({ ...d, angleTune: v })} onChangeEnd={(v: number) => set({ angleTune: v })} />
            <Selector label="スリープまで" options={SLEEP} value={String(d.sleepSec)} onChange={v => set({ sleepSec: Number(v) })} />
          </VStack>
        </Card>
      </div>
      <Text color="secondary" size="sm">{d.model} / FW {d.firmware}</Text>
    </VStack>
  )
}
