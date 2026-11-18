import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
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
import { api, mm, PROFILES, type Key, type KbData, type KeyCode, type Rt } from './api'

const WIDTH: Record<string, number> = {
  Tab: 1.5, Caps: 1.75, Shift: 2.25, RShift: 1.75, Space: 4.25, Enter: 1.5, BS: 1.25,
}
const UNIT = 50
const SOCD_MODES = ['OFF', 'ニュートラル', '先押し優先', '後押し優先', 'キー1優先', 'キー2優先']

type Edit = {
  pointOn: number; pointOff: number; rapidTrigger: boolean; rt: Rt
  keycode: string; fnKeycode: string
}

export function KeyboardPanel({ onToast }: { onToast: (msg: string, err?: boolean) => void }) {
  const [data, setData] = useState<KbData | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [profile, setProfile] = useState<number | null>(null)
  const [sel, setSel] = useState<number[]>([])
  const [edit, setEdit] = useState<Edit | null>(null)
  const [busy, setBusy] = useState(false)
  const [codes, setCodes] = useState<KeyCode[]>([])
  const [live, setLive] = useState(false)
  const [travel, setTravel] = useState<Record<number, { depth: number; on: boolean }>>({})

  const load = useCallback(async (p?: number | null) => {
    try {
      const d = await api.get<KbData>(p == null ? '/api/keyboard' : `/api/keyboard?profile=${p}`)
      setData(d)
      setProfile(d.state.profile)
      setError(null)
    } catch (e) {
      setError((e as Error).message)
    }
  }, [])

  useEffect(() => { load(); api.get<KeyCode[]>('/api/keycodes').then(setCodes).catch(() => {}) }, [load])

  // ライブ表示（押し込み量）
  const liveRef = useRef(live)
  liveRef.current = live
  useEffect(() => {
    if (!live) { setTravel({}); return }
    let stop = false
    const tick = async () => {
      while (!stop && liveRef.current) {
        try {
          const t = await api.get<{ index: number; depth: number; on: boolean }[]>('/api/keyboard/travel')
          const m: Record<number, { depth: number; on: boolean }> = {}
          t.forEach(x => { m[x.index] = { depth: x.depth, on: x.on } })
          setTravel(m)
        } catch { /* 無視 */ }
        await new Promise(r => setTimeout(r, 80))
      }
    }
    tick()
    return () => { stop = true }
  }, [live])

  // 枠の幅に合わせてキーの大きさを決める（最長行 ≒ 15.25 キー分）
  const kbRef = useRef<HTMLDivElement>(null)
  const [unit, setUnit] = useState(UNIT)
  useEffect(() => {
    const el = kbRef.current
    if (!el) return
    const ro = new ResizeObserver(() => setUnit(Math.min(UNIT, Math.max(28, (el.clientWidth - 15 * 4) / 15.25))))
    ro.observe(el)
    return () => ro.disconnect()
  }, [data])

  const keys = data?.state.keys ?? []
  const rows = useMemo(() => {
    const r: Key[][] = []
    keys.forEach(k => { (r[k.row] ??= []).push(k) })
    return r
  }, [keys])

  // 選択が変わったら編集値を先頭キーから作る
  useEffect(() => {
    const first = keys.find(k => k.index === sel[0])
    if (!first) { setEdit(null); return }
    setEdit({
      pointOn: first.pointOn, pointOff: first.pointOff, rapidTrigger: first.rapidTrigger,
      rt: { ...first.rt }, keycode: '', fnKeycode: '',
    })
  }, [sel, data]) // eslint-disable-line react-hooks/exhaustive-deps

  const toggle = (i: number, e: React.MouseEvent) => {
    if (e.shiftKey || e.ctrlKey) setSel(s => (s.includes(i) ? s.filter(x => x !== i) : [...s, i]))
    else setSel(s => (s.length === 1 && s[0] === i ? [] : [i]))
  }
  const pick = (labels: string[]) => setSel(keys.filter(k => labels.includes(k.label)).map(k => k.index))

  const apply = async () => {
    if (!edit || !data) return
    setBusy(true)
    try {
      const body = {
        profile,
        keys: sel.map(index => ({
          index,
          pointOn: edit.pointOn, pointOff: edit.pointOff, rapidTrigger: edit.rapidTrigger,
          rt: edit.rt,
          ...(sel.length === 1 && edit.keycode ? { keycode: edit.keycode } : {}),
          ...(sel.length === 1 && edit.fnKeycode ? { fnKeycode: edit.fnKeycode } : {}),
        })),
      }
      const d = await api.patch<KbData>('/api/keyboard', body)
      setData(d)
      onToast(`${sel.length}キーに保存しました`)
    } catch (e) {
      onToast((e as Error).message, true)
    } finally {
      setBusy(false)
    }
  }

  const patch = async (body: object, msg: string) => {
    setBusy(true)
    try {
      const d = await api.patch<KbData>('/api/keyboard', { profile, ...body })
      setData(d)
      onToast(msg)
    } catch (e) {
      onToast((e as Error).message, true)
    } finally {
      setBusy(false)
    }
  }

  if (error) {
    return (
      <Banner status="warning" title="キーボードに接続できません" description={error}
        endContent={<Button label="再試行" variant="secondary" onClick={() => load(profile)} />} />
    )
  }
  if (!data) return <Text color="secondary">読み込み中…</Text>

  const st = data.state
  const keyOptions = codes.map(c => ({ value: c.name, label: `${c.label}  (${c.name})` }))
  const labelOf = (i: number) => keys.find(k => k.index === i)?.label ?? '-'
  const keyChoices = keys.map(k => ({ value: k.label, label: k.label }))

  return (
    <VStack gap={4}>
      <HStack gap={3} vAlign="center" justify="between">
        <SegmentedControl label="プロファイル" value={String(profile ?? 0)}
          onChange={v => { setSel([]); load(Number(v)) }}>
          {PROFILES.map((n, i) => (
            <SegmentedControlItem key={i} value={String(i)} label={i === data.info.activeProfile ? `${n} ●` : n} />
          ))}
        </SegmentedControl>
        {profile !== data.info.activeProfile && (
          <Button label="このプロファイルを使う" variant="secondary" isDisabled={busy}
            onClick={() => patch({ activeProfile: profile }, 'プロファイルを切り替えました')} />
        )}
      </HStack>

      <div className="cols">
        <Card padding={4}>
          <VStack gap={3}>
            <div className="kb" ref={kbRef}>
              {rows.map((row, ri) => (
                <div className="kb-row" key={ri}>
                  {row.map(k => {
                    const t = travel[k.index]
                    const w = (WIDTH[k.label] ?? 1) * unit
                    const depth = t ? Math.max(0, Math.min(1, t.depth / 1800)) : 0
                    return (
                      <button key={k.index} type="button"
                        className={`key${sel.includes(k.index) ? ' sel' : ''}${k.rapidTrigger ? ' rt' : ''}${t?.on ? ' on' : ''}`}
                        style={{ width: w }} onClick={e => toggle(k.index, e)}
                        title={`#${k.index}  ON ${mm(k.pointOn)} / OFF ${mm(k.pointOff)}`}>
                        {t && <div className="fill" style={{ height: `${depth * 100}%` }} />}
                        <span>{k.label}</span>
                        <span className="ap">{t ? mm(t.depth) : k.rapidTrigger ? `RT ${(k.rt.strokeOn / 1000).toFixed(2)}` : (k.pointOn / 1000).toFixed(2)}</span>
                      </button>
                    )
                  })}
                </div>
              ))}
            </div>
            <HStack gap={2} vAlign="center" justify="between">
              <HStack gap={2}>
                <Button label="WASD" size="sm" variant="secondary" onClick={() => pick(['W', 'A', 'S', 'D'])} />
                <Button label="全キー" size="sm" variant="secondary" onClick={() => setSel(keys.map(k => k.index))} />
                <Button label="解除" size="sm" variant="ghost" onClick={() => setSel([])} />
              </HStack>
              <Switch label="押し込み量を表示" value={live} onChange={setLive} size="sm" />
            </HStack>
            <div className="legend">
              <span>数字 = ON 位置(mm)</span><span><b>RT</b> = ラピッドトリガー感度</span><span>Shift/Ctrl+クリックで複数選択</span>
            </div>
          </VStack>
        </Card>

        <Card padding={4}>
          {!edit ? (
            <Text color="secondary">キーを選択してください</Text>
          ) : (
            <VStack gap={4}>
              <Heading level={3}>{sel.length === 1 ? `${labelOf(sel[0])} キー` : `${sel.length} キー`}</Heading>
              <SegmentedControl label="動作" value={edit.rapidTrigger ? 'rt' : 'normal'}
                onChange={(v: string) => setEdit({ ...edit, rapidTrigger: v === 'rt' })} layout="fill">
                <SegmentedControlItem value="normal" label="通常" />
                <SegmentedControlItem value="rt" label="ラピッドトリガー" />
              </SegmentedControl>
              <Slider label="ON 位置" min={50} max={1800} step={10} value={edit.pointOn} formatValue={mm} valueDisplay="text"
                onChange={(v: number) => setEdit({ ...edit, pointOn: v, pointOff: Math.min(edit.pointOff, v - 10) })} />
              <Slider label="OFF 位置" description="ON 位置より浅く。浅いほど早く離れる" min={0} max={Math.max(0, edit.pointOn - 10)} step={10}
                value={Math.min(edit.pointOff, edit.pointOn - 10)} formatValue={mm} valueDisplay="text"
                onChange={(v: number) => setEdit({ ...edit, pointOff: v })} />
              {edit.rapidTrigger && (
                <>
                  <Slider label="押し込み感度" min={10} max={1000} step={10} value={edit.rt.strokeOn} formatValue={mm} valueDisplay="text"
                    onChange={(v: number) => setEdit({ ...edit, rt: { ...edit.rt, strokeOn: v } })} />
                  <Slider label="戻り感度" min={10} max={1000} step={10} value={edit.rt.strokeOff} formatValue={mm} valueDisplay="text"
                    onChange={(v: number) => setEdit({ ...edit, rt: { ...edit.rt, strokeOff: v } })} />
                  <Slider label="RT 範囲" description="RT が働く深さの幅" min={0} max={1800} step={10} value={edit.rt.range} formatValue={mm} valueDisplay="text"
                    onChange={(v: number) => setEdit({ ...edit, rt: { ...edit.rt, range: v } })} />
                  <Switch label="ビヨンドモード" description="範囲外でも RT を継続" value={edit.rt.beyond}
                    onChange={(v: boolean) => setEdit({ ...edit, rt: { ...edit.rt, beyond: v } })} />
                </>
              )}
              {sel.length === 1 && (
                <>
                  <Selector label="割り当て" placeholder={labelOf(sel[0])} options={keyOptions} value={edit.keycode || null}
                    hasClear onChange={(v: string | null) => setEdit({ ...edit, keycode: v ?? '' })} />
                  <Selector label="Fn 割り当て" placeholder={keys.find(k => k.index === sel[0])?.fnLabel} options={keyOptions}
                    value={edit.fnKeycode || null} hasClear onChange={(v: string | null) => setEdit({ ...edit, fnKeycode: v ?? '' })} />
                </>
              )}
              <Button label="保存" variant="primary" isLoading={busy} onClick={apply} />
            </VStack>
          )}
        </Card>
      </div>

      <div className="cols">
        <Card padding={4}>
          <VStack gap={3}>
            <Heading level={3}>SOCD</Heading>
            <Text color="secondary" size="sm">同時押し時の優先（例: A と D）</Text>
            {st.socd.map((s, i) => (
              <HStack key={i} gap={2} vAlign="end">
                <Selector label={`組 ${i + 1}`} width={150} options={SOCD_MODES.map((m, j) => ({ value: String(j), label: m }))}
                  value={String(s.mode)} onChange={v => patch({ socd: st.socd.map((x, j) => ({ mode: j === i ? Number(v) : x.mode, key1: labelOf(x.key1), key2: labelOf(x.key2) })) }, 'SOCD を保存しました')} />
                <Selector label="キー1" width={100} options={keyChoices} value={labelOf(s.key1)}
                  onChange={v => patch({ socd: st.socd.map((x, j) => ({ mode: x.mode, key1: j === i ? v : labelOf(x.key1), key2: labelOf(x.key2) })) }, 'SOCD を保存しました')} />
                <Selector label="キー2" width={100} options={keyChoices} value={labelOf(s.key2)}
                  onChange={v => patch({ socd: st.socd.map((x, j) => ({ mode: x.mode, key1: labelOf(x.key1), key2: j === i ? v : labelOf(x.key2) })) }, 'SOCD を保存しました')} />
              </HStack>
            ))}
          </VStack>
        </Card>
        <Card padding={4}>
          <HoldTime value={st.holdTime} busy={busy} onSave={h => patch({ holdTime: h }, '保持時間を保存しました')} />
        </Card>
      </div>
      <Text color="secondary" size="sm">{data.info.model} / FW {data.info.firmware}</Text>
    </VStack>
  )
}

function HoldTime({ value, busy, onSave }: { value: { on: number; off: number } | null; busy: boolean; onSave: (v: { on: number; off: number }) => void }) {
  const [v, setV] = useState(value ?? { on: 0, off: 0 })
  useEffect(() => { if (value) setV(value) }, [value])
  if (!value) return <Text color="secondary">このファームウェアは未対応です</Text>
  return (
    <VStack gap={3}>
      <Heading level={3}>チャタリング対策</Heading>
      <Text color="secondary" size="sm">判定が切り替わるまでの保持時間（全キー共通）。誤入力が出るなら少し上げる</Text>
      <HStack gap={2}>
        <NumberInput label="ON 保持" value={v.on} min={0} max={1000} isIntegerOnly width={110} onChange={x => setV({ ...v, on: x })} />
        <NumberInput label="OFF 保持" value={v.off} min={0} max={1000} isIntegerOnly width={110} onChange={x => setV({ ...v, off: x })} />
      </HStack>
      <Button label="保存" variant="secondary" isLoading={busy} onClick={() => onSave(v)} />
    </VStack>
  )
}
