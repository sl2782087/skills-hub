import { memo, useState, type MouseEvent } from 'react'
import { ChevronDown, ChevronRight, Copy, ExternalLink, Github, RefreshCw, Trash2 } from 'lucide-react'
import { openUrl } from '@tauri-apps/plugin-opener'
import { toast } from 'sonner'
import type { TFunction } from 'i18next'
import type { ManagedSkill, ToolOption } from './types'

type GithubInfo = {
  label: string
  href: string
}

type SkillCardProps = {
  skill: ManagedSkill
  installedTools: ToolOption[]
  loading: boolean
  getGithubInfo: (url: string | null | undefined) => GithubInfo | null
  getGithubOpenUrl: (skill: ManagedSkill) => string | null
  getSkillSourceLabel: (skill: ManagedSkill) => string
  formatRelative: (ms: number | null | undefined) => string
  onUpdate: (skill: ManagedSkill) => void
  onDelete: (skillId: string) => void
  onToggleTool: (skill: ManagedSkill, toolId: string) => void
  onSyncNow: (skill: ManagedSkill) => void
  onOpenDetail: (skill: ManagedSkill) => void
  t: TFunction
}

const AVATAR_COLORS: [string, string][] = [
  ['#6366f1', '#8b5cf6'],
  ['#06b6d4', '#3b82f6'],
  ['#10b981', '#14b8a6'],
  ['#f59e0b', '#ef4444'],
  ['#ec4899', '#a855f7'],
]

function getAvatarColors(name: string): [string, string] {
  let hash = 0
  for (let i = 0; i < name.length; i++) hash = (hash * 31 + name.charCodeAt(i)) | 0
  return AVATAR_COLORS[Math.abs(hash) % AVATAR_COLORS.length]!
}

function getInitials(name: string): string {
  const words = name.replace(/[-_]/g, ' ').split(' ').filter(Boolean)
  if (words.length >= 2) return (words[0]![0]! + words[1]![0]!).toUpperCase()
  return name.slice(0, 2).toUpperCase()
}

const MAX_AVATAR_PREVIEW = 3

const SkillCard = ({
  skill,
  installedTools,
  loading,
  getGithubInfo,
  getGithubOpenUrl,
  getSkillSourceLabel,
  formatRelative,
  onUpdate,
  onDelete,
  onToggleTool,
  onSyncNow,
  onOpenDetail,
  t,
}: SkillCardProps) => {
  const [expanded, setExpanded] = useState(false)
  const typeKey = skill.source_type.toLowerCase()
  const isGit = typeKey.includes('git')
  const github = getGithubInfo(skill.source_ref)
  const githubOpenUrl = getGithubOpenUrl(skill)
  const copyValue = (github?.href ?? skill.source_ref ?? '').trim()
  const [c1, c2] = getAvatarColors(skill.name)
  const initials = getInitials(skill.name)

  const syncedToolIds = new Set(skill.targets.map((tgt) => tgt.tool))
  const syncedTools = installedTools.filter((t) => syncedToolIds.has(t.id))
  const allSynced = installedTools.length > 0 && syncedTools.length === installedTools.length
  const statusClass = syncedTools.length === 0 ? 'none' : allSynced ? 'full' : 'partial'
  const previewTools = syncedTools.slice(0, MAX_AVATAR_PREVIEW)
  const extraCount = syncedTools.length - MAX_AVATAR_PREVIEW

  const handleCopy = async () => {
    if (!copyValue) return
    try {
      await navigator.clipboard.writeText(copyValue)
      toast.success(t('copied'))
    } catch {
      toast.error(t('copyFailed'))
    }
  }

  const handleOpenGithub = (e: MouseEvent) => {
    e.stopPropagation()
    if (!githubOpenUrl) return
    void (async () => {
      try {
        await openUrl(githubOpenUrl)
      } catch {
        window.open(githubOpenUrl, '_blank', 'noopener,noreferrer')
      }
    })()
  }

  const handleDetailClick = (e: MouseEvent) => {
    e.stopPropagation()
    onOpenDetail(skill)
  }

  const handleExpandClick = (e: MouseEvent) => {
    e.stopPropagation()
    setExpanded((v) => !v)
  }

  return (
    <div className={`skill-card-v2${expanded ? ' expanded' : ''}`}>
      {/* Header row */}
      <div
        className="skill-card-header"
        role="button"
        tabIndex={0}
        aria-expanded={expanded}
        onClick={() => setExpanded((v) => !v)}
        onKeyDown={(e) => {
          if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault()
            setExpanded((v) => !v)
          }
        }}
      >
        {/* Avatar */}
        <div
          className="skill-avatar"
          style={{ background: `linear-gradient(135deg, ${c1}, ${c2})` }}
          aria-hidden="true"
        >
          {initials}
        </div>

        {/* Info */}
        <div className="skill-card-info">
          <div className="skill-card-name-row">
            <span className="skill-card-name">{skill.name}</span>
            <span className={`skill-source-tag ${isGit ? 'git' : 'local'}`}>
              {isGit ? 'git' : 'local'}
            </span>
          </div>
          <div className="skill-card-meta">
            {github ? github.label : getSkillSourceLabel(skill)}
            <span className="meta-dot">·</span>
            {formatRelative(skill.updated_at)}
          </div>
        </div>

        {/* Right: tool preview + status + icon buttons + expand arrow */}
        <div className="skill-card-right">
          {previewTools.length > 0 && (
            <div className="tool-avatar-row">
              {previewTools.map((tool) => (
                <div key={tool.id} className="tool-avatar" title={tool.label}>
                  {tool.label.slice(0, 2).toUpperCase()}
                </div>
              ))}
              {extraCount > 0 && (
                <div className="tool-avatar extra">+{extraCount}</div>
              )}
            </div>
          )}
          <span className={`sync-badge ${statusClass}`}>
            {statusClass === 'full'
              ? t('skillBadgeSynced')
              : statusClass === 'partial'
              ? t('skillBadgePartial', { count: syncedTools.length })
              : '—'}
          </span>
          <button
            type="button"
            className="skill-card-icon-btn"
            title={t('skillCardDetail')}
            aria-label={t('skillCardDetail')}
            onClick={handleDetailClick}
          >
            <ExternalLink size={14} />
          </button>
          {githubOpenUrl ? (
            <button
              type="button"
              className="skill-card-icon-btn"
              title={t('openSkillOnGithub')}
              aria-label={t('openSkillOnGithubAria')}
              onClick={handleOpenGithub}
            >
              <Github size={14} />
            </button>
          ) : null}
          <button
            type="button"
            className="skill-card-expand-btn"
            aria-label={expanded ? t('collapse') : t('expand')}
            onClick={handleExpandClick}
          >
            {expanded ? <ChevronDown size={16} /> : <ChevronRight size={16} />}
          </button>
        </div>
      </div>

      {/* Sync Panel (expanded) */}
      {expanded && (
        <div className="skill-sync-panel">
          <div className="sync-panel-label">{t('skillSyncPanel')}</div>
          <div className="sync-tool-grid">
            {installedTools.map((tool) => {
              const isSynced = syncedToolIds.has(tool.id)
              return (
                <button
                  key={tool.id}
                  type="button"
                  className={`sync-tool-btn${isSynced ? ' active' : ''}`}
                  onClick={() => void onToggleTool(skill, tool.id)}
                  disabled={loading}
                  title={tool.label}
                >
                  <span className={`sync-tool-dot${isSynced ? ' active' : ''}`} />
                  <span className="sync-tool-label">{tool.label}</span>
                </button>
              )
            })}
          </div>
          <div className="sync-panel-actions">
            <button
              type="button"
              className="btn btn-primary btn-sm"
              onClick={() => onSyncNow(skill)}
              disabled={loading}
            >
              <RefreshCw size={13} />
              {t('syncNow')}
            </button>
            <button
              type="button"
              className="btn btn-secondary btn-sm"
              onClick={() => onUpdate(skill)}
              disabled={loading}
            >
              <RefreshCw size={13} />
              {t('update')}
            </button>
            {copyValue ? (
              <button
                type="button"
                className="btn btn-secondary btn-sm"
                onClick={() => void handleCopy()}
              >
                <Copy size={13} />
                {t('copy')}
              </button>
            ) : null}
            <button
              type="button"
              className="btn btn-danger btn-sm"
              onClick={() => onDelete(skill.id)}
              disabled={loading}
            >
              <Trash2 size={13} />
              {t('remove')}
            </button>
          </div>
        </div>
      )}
    </div>
  )
}

export default memo(SkillCard)
