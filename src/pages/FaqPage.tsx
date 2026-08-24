import {
  M3eButton,
  M3eDialog,
  M3eDialogAction,
  M3eHeading,
  M3eIcon,
  M3eListItem,
} from "@m3e/react/all";
import { useState, type HTMLAttributes } from "react";
import { useTranslation } from "react-i18next";

const MAX_FAQ_IDX = 18;

export default function FaqPage() {
  const { t } = useTranslation();
  const [selectedId, setSelectedId] = useState<string | null>(null);

  return (
    <div className="h-full w-full flex flex-col overflow-hidden">
      <div className="flex-1 min-h-0 overflow-y-auto p-6 scrollbar-thin">
        <div className="mx-auto flex w-full max-w-3xl flex-col gap-4">
          {/* 标题 */}
          <div className="flex items-center gap-3 px-1">
            <M3eIcon name="help" className="text-2xl text-md-primary" />
            <M3eHeading variant="headline" size="small">
              {t("pages.faq.title")}
            </M3eHeading>
          </div>

          {/* FAQ 条目 */}
          <div className="flex flex-col gap-2">
            {Array.from({ length: MAX_FAQ_IDX }, (_, i) => String(i + 1)).map(
              (id) => (
                <M3eListItem
                  key={id}
                  className="faq-item cursor-pointer"
                  onClick={() => setSelectedId(id)}
                >
                  <M3eIcon slot="leading" name="help_outline" />
                  {t(`pages.faq.q${id}`)}
                  <M3eIcon slot="trailing" name="chevron_right" />
                </M3eListItem>
              ),
            )}
          </div>
        </div>
      </div>

      {/* 答案 Dialog */}
      <M3eDialog
        open={selectedId !== null}
        dismissible
        onClosed={() => setSelectedId(null)}
      >
        <span slot="header">
          {selectedId ? t(`pages.faq.q${selectedId}`) : ""}
        </span>
        {selectedId ? (
          <p className="whitespace-pre-line text-sm leading-relaxed text-md-on-surface-variant">
            {t(`pages.faq.a${selectedId}`)}
          </p>
        ) : null}
        <div
          slot="actions"
          {...({ end: "" } as HTMLAttributes<HTMLDivElement>)}
        >
          <M3eButton>
            <M3eDialogAction return-value="close">
              {t("pages.faq.closeAnswer")}
            </M3eDialogAction>
          </M3eButton>
        </div>
      </M3eDialog>
    </div>
  );
}
