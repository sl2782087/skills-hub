import { memo } from 'react'
import { ArrowUpDown, Plus, RefreshCw, Search } from 'lucide-react'
import type { TFunction } from 'i18next'

type FilterBarProps = {
  sortBy: 'updated' | 'name'
  searchQuery: string
  loading: boolean
  sourceFilter: 'all' | 'git' | 'local'
  managedSkillsCount: number
  onSortChange: (value: 'updated' | 'name') => void
  onSearchChange: (value: string) => void
  onSourceFilterChange: (value: 'all' | 'git' | 'local') => void
  onRefresh: () => void
  onOpenAdd: () => void
  t: TFunction
}

const FilterBar = ({
  sortBy,
  searchQuery,
  loading,
  sourceFilter,
  managedSkillsCount,
  onSortChange,
  onSearchChange,
  onSourceFilterChange,
  onRefresh,
  onOpenAdd,
  t,
}: FilterBarProps) => {
  return (
    <div className="filter-bar-v2">
      <div className="filter-bar-row">
        <div className="search-container">
          <Search size={15} className="search-icon-abs" />
          <input
            className="search-input"
            value={searchQuery}
            onChange={(event) => onSearchChange(event.target.value)}
            placeholder={t('searchPlaceholder')}
          />
        </div>
        <div className="filter-chips">
          {(['all', 'git', 'local'] as const).map((val) => (
            <button
              key={val}
              type="button"
              className={`filter-chip${sourceFilter === val ? ' active' : ''}`}
              onClick={() => onSourceFilterChange(val)}
            >
              {val === 'all'
                ? `${t('filterAll')}(${managedSkillsCount})`
                : val === 'git'
                ? t('filterGit')
                : t('filterLocal')}
            </button>
          ))}
        </div>
        <button className="btn btn-secondary sort-btn" type="button">
          <span className="sort-label">{t('filterSort')}:</span>
          {sortBy === 'updated' ? t('sortUpdated') : t('sortName')}
          <ArrowUpDown size={12} />
          <select
            aria-label={t('filterSort')}
            value={sortBy}
            onChange={(event) => onSortChange(event.target.value as 'updated' | 'name')}
          >
            <option value="updated">{t('sortUpdated')}</option>
            <option value="name">{t('sortName')}</option>
          </select>
        </button>
        <button
          className="btn btn-secondary"
          type="button"
          onClick={onRefresh}
          disabled={loading}
          aria-label={t('refresh')}
        >
          <RefreshCw size={14} />
        </button>
        <button
          className="btn btn-primary"
          type="button"
          onClick={onOpenAdd}
          disabled={loading}
        >
          <Plus size={14} />
          {t('addSkill')}
        </button>
      </div>
    </div>
  )
}

export default memo(FilterBar)
