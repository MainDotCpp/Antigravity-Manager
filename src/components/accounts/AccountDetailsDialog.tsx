import { X, Clock, AlertCircle, Bot, Lock, Unlock } from 'lucide-react';
import { createPortal } from 'react-dom';
import { Account } from '../../types/account';
import { formatDate } from '../../utils/format';
import { useTranslation } from 'react-i18next';
import { MODEL_CONFIG, sortModels } from '../../config/modelConfig';

interface AccountDetailsDialogProps {
    account: Account | null;
    onClose: () => void;
    onUnlock?: (accountId: string, model: string) => Promise<void>;
}

export default function AccountDetailsDialog({ account, onClose, onUnlock }: AccountDetailsDialogProps) {
    const { t } = useTranslation();
    if (!account) return null;

    return createPortal(
        <div className="modal modal-open z-[100]">
            {/* Draggable Top Region */}
            <div data-tauri-drag-region className="fixed top-0 left-0 right-0 h-8 z-[110]" />

            <div className="modal-box relative max-w-3xl bg-white dark:bg-base-100 shadow-2xl rounded-2xl p-0 overflow-hidden">
                {/* Header */}
                <div className="px-6 py-5 border-b border-gray-100 dark:border-base-200 bg-gray-50/50 dark:bg-base-200/50 flex justify-between items-center">
                    <div className="flex items-center gap-3">
                        <h3 className="font-bold text-lg text-gray-900 dark:text-base-content">{t('accounts.details.title')}</h3>
                        <div className="px-2.5 py-0.5 rounded-full bg-gray-100 dark:bg-base-200 border border-gray-200 dark:border-base-300 text-xs font-mono text-gray-500 dark:text-gray-400">
                            {account.email}
                        </div>
                        {account.quota?.subscription_tier && (
                            <div className={`px-2 py-0.5 rounded text-[10px] font-bold uppercase tracking-wider ${account.quota.subscription_tier === 'ultra' ? 'bg-purple-100 text-purple-700 dark:bg-purple-900/30 dark:text-purple-400' :
                                account.quota.subscription_tier === 'pro' ? 'bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-400' : 'bg-gray-100 text-gray-600 dark:bg-base-300 dark:text-gray-400'
                                }`}>
                                {account.quota.subscription_tier}
                            </div>
                        )}
                    </div>
                    <button
                        onClick={onClose}
                        className="btn btn-sm btn-circle btn-ghost text-gray-400 hover:bg-gray-100 dark:hover:bg-base-200 hover:text-gray-600 dark:hover:text-base-content transition-colors"
                    >
                        <X size={18} />
                    </button>
                </div>

                {/* Status Alerts */}
                {(account.disabled || account.proxy_disabled) && (
                    <div className="px-6 py-3 bg-red-50 dark:bg-red-950/20 border-b border-red-100 dark:border-red-900/30 flex flex-col gap-1">
                        {account.disabled && (
                            <div className="flex items-center gap-2 text-xs text-red-700 dark:text-red-400">
                                <AlertCircle size={14} />
                                <span className="font-semibold">{t('accounts.status.disabled')}:</span>
                                <span>{account.disabled_reason || t('common.unknown')}</span>
                            </div>
                        )}
                        {account.proxy_disabled && (
                            <div className="flex items-center gap-2 text-xs text-orange-700 dark:text-orange-400">
                                <AlertCircle size={14} />
                                <span className="font-semibold">{t('accounts.status.proxy_disabled')}:</span>
                                <span>{account.proxy_disabled_reason || t('common.unknown')}</span>
                            </div>
                        )}
                    </div>
                )}

                {/* Content */}
                <div className="p-6 max-h-[60vh] overflow-y-auto">
                    {/* Protected Models Section */}
                    {account.protected_models && Object.keys(account.protected_models).length > 0 && (
                        <div className="mb-6">
                            <h4 className="text-xs font-bold text-gray-500 dark:text-gray-400 uppercase tracking-widest mb-3 flex items-center gap-2">
                                <Lock size={12} className="text-amber-500" />
                                {t('accounts.details.protected_models')}
                            </h4>
                            <div className="flex flex-col gap-2">
                                {Object.entries(account.protected_models).map(([model, protection]) => {
                                    const now = Math.floor(Date.now() / 1000);
                                    const hasUnlockTime = protection.unlocks_at != null;
                                    const remaining = hasUnlockTime ? (protection.unlocks_at! - now) : 0;
                                    const isExpired = hasUnlockTime && remaining <= 0;

                                    const reasonLabel = protection.reason === 'validation_required' ? t('accounts.protection.validation_required', 'Validation Required')
                                        : protection.reason === 'quota_exhausted' ? t('accounts.protection.quota_exhausted', 'Quota Exhausted')
                                        : protection.reason === 'manual' ? t('accounts.protection.manual', 'Manual Lock')
                                        : protection.reason === 'legacy_conversion' ? t('accounts.protection.legacy', 'Legacy')
                                        : protection.reason;

                                    const formatCountdown = (secs: number) => {
                                        if (secs <= 0) return t('accounts.protection.expired', 'Expired');
                                        const h = Math.floor(secs / 3600);
                                        const m = Math.floor((secs % 3600) / 60);
                                        if (h > 0) return `${h}h ${m}m`;
                                        return `${m}m`;
                                    };

                                    return (
                                        <div key={model} className="flex items-center justify-between px-3 py-2 bg-amber-50/50 dark:bg-amber-900/10 border border-amber-100 dark:border-amber-900/30 rounded-lg">
                                            <div className="flex items-center gap-2 min-w-0">
                                                <Lock size={12} className="text-amber-500 shrink-0" />
                                                <span className="text-[11px] font-mono font-medium text-amber-700 dark:text-amber-400 truncate">{model}</span>
                                                <span className={`text-[9px] font-bold px-1.5 py-0.5 rounded-md ${
                                                    protection.reason === 'validation_required' ? 'bg-red-100 dark:bg-red-900/30 text-red-600 dark:text-red-400' :
                                                    protection.reason === 'quota_exhausted' ? 'bg-orange-100 dark:bg-orange-900/30 text-orange-600 dark:text-orange-400' :
                                                    'bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-400'
                                                }`}>{reasonLabel}</span>
                                            </div>
                                            <div className="flex items-center gap-2 shrink-0">
                                                {hasUnlockTime && !isExpired && (
                                                    <span className="flex items-center gap-1 text-[10px] font-mono text-amber-600 dark:text-amber-400">
                                                        <Clock size={10} />
                                                        {formatCountdown(remaining)}
                                                    </span>
                                                )}
                                                {isExpired && (
                                                    <span className="flex items-center gap-1 text-[10px] font-mono text-green-600 dark:text-green-400">
                                                        <Unlock size={10} />
                                                        {t('accounts.protection.expired', 'Expired')}
                                                    </span>
                                                )}
                                                {!hasUnlockTime && (
                                                    <span className="text-[10px] font-mono text-gray-400 dark:text-gray-500">
                                                        {t('accounts.protection.no_auto_unlock', 'Manual')}
                                                    </span>
                                                )}
                                                {onUnlock && (
                                                    <button
                                                        onClick={() => onUnlock(account.id, model)}
                                                        className="flex items-center gap-1 text-[10px] px-1.5 py-0.5 rounded text-blue-600 dark:text-blue-400 hover:bg-blue-50 dark:hover:bg-blue-900/30 transition-colors"
                                                        title={t('accounts.protection.unlock', 'Unlock')}
                                                    >
                                                        <Unlock size={10} />
                                                        {t('accounts.protection.unlock', 'Unlock')}
                                                    </button>
                                                )}
                                            </div>
                                        </div>
                                    );
                                })}
                            </div>
                        </div>
                    )}

                    <h4 className="text-xs font-bold text-gray-500 dark:text-gray-400 uppercase tracking-widest mb-3">{t('accounts.details.model_quota')}</h4>
                    <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                        {(() => {
                            const uniqueLabels = new Set<string>();
                            return sortModels(
                                (account.quota?.models || []).map(model => {
                                    const config = MODEL_CONFIG[model.name.toLowerCase()];
                                    const label = model.display_name || (config?.i18nKey ? t(config.i18nKey) : (config?.label || model.name));
                                    return {
                                        id: model.name.toLowerCase(),
                                        label: label,
                                        model
                                    };
                                })
                            ).filter(m => {
                                if (uniqueLabels.has(m.label)) return false;
                                uniqueLabels.add(m.label);
                                return true;
                            }).map(({ model, label }) => (
                                <div key={model.name} className="p-4 rounded-xl border border-gray-100 dark:border-base-200 bg-white dark:bg-base-100 hover:border-blue-100 dark:hover:border-blue-900 hover:shadow-sm transition-all group">
                                    <div className="flex justify-between items-start mb-3">
                                        <div className="flex flex-col gap-1">
                                            <div className="flex items-center gap-2">
                                                {(() => {
                                                    const Icon = MODEL_CONFIG[model.name.toLowerCase()]?.Icon || Bot;
                                                    return <Icon size={16} className="shrink-0" />;
                                                })()}
                                                <span className="text-sm font-medium font-mono text-gray-700 dark:text-gray-300 group-hover:text-blue-700 dark:group-hover:text-blue-400 transition-colors">
                                                    {label}
                                                </span>
                                            </div>
                                            {model.thinking_budget !== undefined && (
                                                <span className="text-[10px] text-gray-500 font-mono bg-gray-100 dark:bg-base-200 px-1 rounded inline-block w-max mt-0.5">
                                                    {t('proxy.config.thinking_budget', 'Thinking Budget')}: {model.thinking_budget}
                                                </span>
                                            )}
                                        </div>
                                        <span
                                            className={`text-xs font-bold px-2 py-0.5 rounded-md ${model.percentage >= 50 ? 'bg-green-50 text-green-700 dark:bg-green-900/30 dark:text-green-400' :
                                                model.percentage >= 20 ? 'bg-orange-50 text-orange-700 dark:bg-orange-900/30 dark:text-orange-400' :
                                                    'bg-red-50 text-red-700 dark:bg-red-900/30 dark:text-red-400'
                                                }`}
                                        >
                                            {model.percentage}%
                                        </span>
                                    </div>

                                    {/* Progress Bar */}
                                    <div className="h-1.5 w-full bg-gray-100 dark:bg-base-200 rounded-full overflow-hidden mb-3">
                                        <div
                                            className={`h-full rounded-full transition-all duration-500 ${model.percentage >= 50 ? 'bg-emerald-500' :
                                                model.percentage >= 20 ? 'bg-orange-400' :
                                                    'bg-red-500'
                                                }`}
                                            style={{ width: `${model.percentage}%` }}
                                        ></div>
                                    </div>

                                    <div className="flex items-center gap-1.5 text-[10px] text-gray-400 dark:text-gray-500 font-mono">
                                        <Clock size={10} />
                                        <span>{t('accounts.reset_time')}: {formatDate(model.reset_time) || t('common.unknown')}</span>
                                    </div>
                                </div>
                            ));
                        })() || (
                                <div className="col-span-2 py-10 text-center text-gray-400 flex flex-col items-center">
                                    <AlertCircle className="w-8 h-8 mb-2 opacity-20" />
                                    <span>{t('accounts.no_data')}</span>
                                </div>
                            )}
                    </div>
                </div>
            </div>
            <div className="modal-backdrop bg-black/40 backdrop-blur-sm" onClick={onClose}></div>
        </div>,
        document.body
    );
}
