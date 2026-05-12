import {
  MotionModule
} from "./chunk-MEOAIN2L.js";
import {
  Button
} from "./chunk-EVZI6JA5.js";
import "./chunk-MXDPHVA6.js";
import {
  Ripple
} from "./chunk-3DHUXVBZ.js";
import "./chunk-JU7IVVJ4.js";
import {
  PlusIcon,
  TimesIcon,
  UploadIcon
} from "./chunk-6O55ZLZ5.js";
import "./chunk-TKXJWV2M.js";
import {
  Badge
} from "./chunk-ZHLY3VTM.js";
import {
  BaseComponent,
  PARENT_INSTANCE
} from "./chunk-3OBHI6YD.js";
import {
  Bind
} from "./chunk-HI3E3LQB.js";
import {
  BaseStyle
} from "./chunk-X3ZNKE5C.js";
import {
  DomSanitizer
} from "./chunk-QOAWITSL.js";
import "./chunk-H3G5RLZA.js";
import {
  HttpClient,
  HttpEventType
} from "./chunk-C6ACCL6J.js";
import "./chunk-OBQZS5OM.js";
import {
  PrimeTemplate,
  SharedModule,
  TranslationKeys
} from "./chunk-GPM6AXFD.js";
import {
  CommonModule,
  NgClass,
  NgForOf,
  NgIf,
  NgStyle,
  NgTemplateOutlet,
  isPlatformBrowser
} from "./chunk-FXCB4XNR.js";
import "./chunk-CWNTQMSC.js";
import {
  ChangeDetectionStrategy,
  Component,
  ContentChild,
  ContentChildren,
  EventEmitter,
  Injectable,
  InjectionToken,
  Input,
  NgModule,
  NgZone,
  Output,
  ViewChild,
  ViewEncapsulation,
  booleanAttribute,
  computed,
  inject,
  input,
  numberAttribute,
  output,
  setClassMetadata,
  signal,
  ɵɵHostDirectivesFeature,
  ɵɵInheritDefinitionFeature,
  ɵɵProvidersFeature,
  ɵɵadvance,
  ɵɵanimateEnter,
  ɵɵanimateLeave,
  ɵɵattribute,
  ɵɵclassMap,
  ɵɵclassProp,
  ɵɵconditional,
  ɵɵconditionalCreate,
  ɵɵcontentQuery,
  ɵɵdefineComponent,
  ɵɵdefineInjectable,
  ɵɵdefineInjector,
  ɵɵdefineNgModule,
  ɵɵelement,
  ɵɵelementContainer,
  ɵɵelementContainerEnd,
  ɵɵelementContainerStart,
  ɵɵelementEnd,
  ɵɵelementStart,
  ɵɵgetCurrentView,
  ɵɵgetInheritedFactory,
  ɵɵlistener,
  ɵɵloadQuery,
  ɵɵnamespaceSVG,
  ɵɵnextContext,
  ɵɵprojection,
  ɵɵprojectionDef,
  ɵɵproperty,
  ɵɵpureFunction1,
  ɵɵpureFunction3,
  ɵɵpureFunction5,
  ɵɵpureFunction8,
  ɵɵqueryRefresh,
  ɵɵreference,
  ɵɵrepeater,
  ɵɵrepeaterCreate,
  ɵɵrepeaterTrackByIdentity,
  ɵɵresetView,
  ɵɵrestoreView,
  ɵɵsanitizeHtml,
  ɵɵsanitizeUrl,
  ɵɵstyleMap,
  ɵɵstyleProp,
  ɵɵtemplate,
  ɵɵtemplateRefExtractor,
  ɵɵtext,
  ɵɵtextInterpolate,
  ɵɵtextInterpolate1,
  ɵɵtextInterpolate2,
  ɵɵviewQuery
} from "./chunk-M2EDENCT.js";
import "./chunk-JRFR6BLO.js";
import "./chunk-HWYXSU2G.js";
import "./chunk-MARUHEWW.js";
import {
  P,
  W
} from "./chunk-7V5MERLO.js";
import {
  __spreadValues
} from "./chunk-GOMI4DH3.js";

// node_modules/@primeuix/styles/dist/message/index.mjs
var style = "\n    .p-message {\n        display: grid;\n        grid-template-rows: 1fr;\n        border-radius: dt('message.border.radius');\n        outline-width: dt('message.border.width');\n        outline-style: solid;\n    }\n\n    .p-message-content-wrapper {\n        min-height: 0;\n    }\n\n    .p-message-content {\n        display: flex;\n        align-items: center;\n        padding: dt('message.content.padding');\n        gap: dt('message.content.gap');\n    }\n\n    .p-message-icon {\n        flex-shrink: 0;\n    }\n\n    .p-message-close-button {\n        display: flex;\n        align-items: center;\n        justify-content: center;\n        flex-shrink: 0;\n        margin-inline-start: auto;\n        overflow: hidden;\n        position: relative;\n        width: dt('message.close.button.width');\n        height: dt('message.close.button.height');\n        border-radius: dt('message.close.button.border.radius');\n        background: transparent;\n        transition:\n            background dt('message.transition.duration'),\n            color dt('message.transition.duration'),\n            outline-color dt('message.transition.duration'),\n            box-shadow dt('message.transition.duration'),\n            opacity 0.3s;\n        outline-color: transparent;\n        color: inherit;\n        padding: 0;\n        border: none;\n        cursor: pointer;\n        user-select: none;\n    }\n\n    .p-message-close-icon {\n        font-size: dt('message.close.icon.size');\n        width: dt('message.close.icon.size');\n        height: dt('message.close.icon.size');\n    }\n\n    .p-message-close-button:focus-visible {\n        outline-width: dt('message.close.button.focus.ring.width');\n        outline-style: dt('message.close.button.focus.ring.style');\n        outline-offset: dt('message.close.button.focus.ring.offset');\n    }\n\n    .p-message-info {\n        background: dt('message.info.background');\n        outline-color: dt('message.info.border.color');\n        color: dt('message.info.color');\n        box-shadow: dt('message.info.shadow');\n    }\n\n    .p-message-info .p-message-close-button:focus-visible {\n        outline-color: dt('message.info.close.button.focus.ring.color');\n        box-shadow: dt('message.info.close.button.focus.ring.shadow');\n    }\n\n    .p-message-info .p-message-close-button:hover {\n        background: dt('message.info.close.button.hover.background');\n    }\n\n    .p-message-info.p-message-outlined {\n        color: dt('message.info.outlined.color');\n        outline-color: dt('message.info.outlined.border.color');\n    }\n\n    .p-message-info.p-message-simple {\n        color: dt('message.info.simple.color');\n    }\n\n    .p-message-success {\n        background: dt('message.success.background');\n        outline-color: dt('message.success.border.color');\n        color: dt('message.success.color');\n        box-shadow: dt('message.success.shadow');\n    }\n\n    .p-message-success .p-message-close-button:focus-visible {\n        outline-color: dt('message.success.close.button.focus.ring.color');\n        box-shadow: dt('message.success.close.button.focus.ring.shadow');\n    }\n\n    .p-message-success .p-message-close-button:hover {\n        background: dt('message.success.close.button.hover.background');\n    }\n\n    .p-message-success.p-message-outlined {\n        color: dt('message.success.outlined.color');\n        outline-color: dt('message.success.outlined.border.color');\n    }\n\n    .p-message-success.p-message-simple {\n        color: dt('message.success.simple.color');\n    }\n\n    .p-message-warn {\n        background: dt('message.warn.background');\n        outline-color: dt('message.warn.border.color');\n        color: dt('message.warn.color');\n        box-shadow: dt('message.warn.shadow');\n    }\n\n    .p-message-warn .p-message-close-button:focus-visible {\n        outline-color: dt('message.warn.close.button.focus.ring.color');\n        box-shadow: dt('message.warn.close.button.focus.ring.shadow');\n    }\n\n    .p-message-warn .p-message-close-button:hover {\n        background: dt('message.warn.close.button.hover.background');\n    }\n\n    .p-message-warn.p-message-outlined {\n        color: dt('message.warn.outlined.color');\n        outline-color: dt('message.warn.outlined.border.color');\n    }\n\n    .p-message-warn.p-message-simple {\n        color: dt('message.warn.simple.color');\n    }\n\n    .p-message-error {\n        background: dt('message.error.background');\n        outline-color: dt('message.error.border.color');\n        color: dt('message.error.color');\n        box-shadow: dt('message.error.shadow');\n    }\n\n    .p-message-error .p-message-close-button:focus-visible {\n        outline-color: dt('message.error.close.button.focus.ring.color');\n        box-shadow: dt('message.error.close.button.focus.ring.shadow');\n    }\n\n    .p-message-error .p-message-close-button:hover {\n        background: dt('message.error.close.button.hover.background');\n    }\n\n    .p-message-error.p-message-outlined {\n        color: dt('message.error.outlined.color');\n        outline-color: dt('message.error.outlined.border.color');\n    }\n\n    .p-message-error.p-message-simple {\n        color: dt('message.error.simple.color');\n    }\n\n    .p-message-secondary {\n        background: dt('message.secondary.background');\n        outline-color: dt('message.secondary.border.color');\n        color: dt('message.secondary.color');\n        box-shadow: dt('message.secondary.shadow');\n    }\n\n    .p-message-secondary .p-message-close-button:focus-visible {\n        outline-color: dt('message.secondary.close.button.focus.ring.color');\n        box-shadow: dt('message.secondary.close.button.focus.ring.shadow');\n    }\n\n    .p-message-secondary .p-message-close-button:hover {\n        background: dt('message.secondary.close.button.hover.background');\n    }\n\n    .p-message-secondary.p-message-outlined {\n        color: dt('message.secondary.outlined.color');\n        outline-color: dt('message.secondary.outlined.border.color');\n    }\n\n    .p-message-secondary.p-message-simple {\n        color: dt('message.secondary.simple.color');\n    }\n\n    .p-message-contrast {\n        background: dt('message.contrast.background');\n        outline-color: dt('message.contrast.border.color');\n        color: dt('message.contrast.color');\n        box-shadow: dt('message.contrast.shadow');\n    }\n\n    .p-message-contrast .p-message-close-button:focus-visible {\n        outline-color: dt('message.contrast.close.button.focus.ring.color');\n        box-shadow: dt('message.contrast.close.button.focus.ring.shadow');\n    }\n\n    .p-message-contrast .p-message-close-button:hover {\n        background: dt('message.contrast.close.button.hover.background');\n    }\n\n    .p-message-contrast.p-message-outlined {\n        color: dt('message.contrast.outlined.color');\n        outline-color: dt('message.contrast.outlined.border.color');\n    }\n\n    .p-message-contrast.p-message-simple {\n        color: dt('message.contrast.simple.color');\n    }\n\n    .p-message-text {\n        font-size: dt('message.text.font.size');\n        font-weight: dt('message.text.font.weight');\n    }\n\n    .p-message-icon {\n        font-size: dt('message.icon.size');\n        width: dt('message.icon.size');\n        height: dt('message.icon.size');\n    }\n\n    .p-message-sm .p-message-content {\n        padding: dt('message.content.sm.padding');\n    }\n\n    .p-message-sm .p-message-text {\n        font-size: dt('message.text.sm.font.size');\n    }\n\n    .p-message-sm .p-message-icon {\n        font-size: dt('message.icon.sm.size');\n        width: dt('message.icon.sm.size');\n        height: dt('message.icon.sm.size');\n    }\n\n    .p-message-sm .p-message-close-icon {\n        font-size: dt('message.close.icon.sm.size');\n        width: dt('message.close.icon.sm.size');\n        height: dt('message.close.icon.sm.size');\n    }\n\n    .p-message-lg .p-message-content {\n        padding: dt('message.content.lg.padding');\n    }\n\n    .p-message-lg .p-message-text {\n        font-size: dt('message.text.lg.font.size');\n    }\n\n    .p-message-lg .p-message-icon {\n        font-size: dt('message.icon.lg.size');\n        width: dt('message.icon.lg.size');\n        height: dt('message.icon.lg.size');\n    }\n\n    .p-message-lg .p-message-close-icon {\n        font-size: dt('message.close.icon.lg.size');\n        width: dt('message.close.icon.lg.size');\n        height: dt('message.close.icon.lg.size');\n    }\n\n    .p-message-outlined {\n        background: transparent;\n        outline-width: dt('message.outlined.border.width');\n    }\n\n    .p-message-simple {\n        background: transparent;\n        outline-color: transparent;\n        box-shadow: none;\n    }\n\n    .p-message-simple .p-message-content {\n        padding: dt('message.simple.content.padding');\n    }\n\n    .p-message-outlined .p-message-close-button:hover,\n    .p-message-simple .p-message-close-button:hover {\n        background: transparent;\n    }\n\n    .p-message-enter-active {\n        animation: p-animate-message-enter 0.3s ease-out forwards;\n        overflow: hidden;\n    }\n\n    .p-message-leave-active {\n        animation: p-animate-message-leave 0.15s ease-in forwards;\n        overflow: hidden;\n    }\n\n    @keyframes p-animate-message-enter {\n        from {\n            opacity: 0;\n            grid-template-rows: 0fr;\n        }\n        to {\n            opacity: 1;\n            grid-template-rows: 1fr;\n        }\n    }\n\n    @keyframes p-animate-message-leave {\n        from {\n            opacity: 1;\n            grid-template-rows: 1fr;\n        }\n        to {\n            opacity: 0;\n            margin: 0;\n            grid-template-rows: 0fr;\n        }\n    }\n";

// node_modules/primeng/fesm2022/primeng-message.mjs
var _c0 = ["container"];
var _c1 = ["icon"];
var _c2 = ["closeicon"];
var _c3 = ["*"];
var _c4 = (a0) => ({
  closeCallback: a0
});
function Message_Conditional_2_ng_container_0_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵelementContainer(0);
  }
}
function Message_Conditional_2_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵtemplate(0, Message_Conditional_2_ng_container_0_Template, 1, 0, "ng-container", 4);
  }
  if (rf & 2) {
    const ctx_r0 = ɵɵnextContext();
    ɵɵproperty("ngTemplateOutlet", ctx_r0.iconTemplate || ctx_r0._iconTemplate);
  }
}
function Message_Conditional_3_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵelement(0, "i", 1);
  }
  if (rf & 2) {
    const ctx_r0 = ɵɵnextContext();
    ɵɵclassMap(ctx_r0.cn(ctx_r0.cx("icon"), ctx_r0.icon));
    ɵɵproperty("pBind", ctx_r0.ptm("icon"));
    ɵɵattribute("data-p", ctx_r0.dataP);
  }
}
function Message_Conditional_4_ng_container_0_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵelementContainer(0);
  }
}
function Message_Conditional_4_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵtemplate(0, Message_Conditional_4_ng_container_0_Template, 1, 0, "ng-container", 5);
  }
  if (rf & 2) {
    const ctx_r0 = ɵɵnextContext();
    ɵɵproperty("ngTemplateOutlet", ctx_r0.containerTemplate || ctx_r0._containerTemplate)("ngTemplateOutletContext", ɵɵpureFunction1(2, _c4, ctx_r0.closeCallback));
  }
}
function Message_Conditional_5_div_0_span_1_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵelement(0, "span", 9);
  }
  if (rf & 2) {
    const ctx_r0 = ɵɵnextContext(3);
    ɵɵproperty("pBind", ctx_r0.ptm("text"))("ngClass", ctx_r0.cx("text"))("innerHTML", ctx_r0.text, ɵɵsanitizeHtml);
    ɵɵattribute("data-p", ctx_r0.dataP);
  }
}
function Message_Conditional_5_div_0_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵelementStart(0, "div");
    ɵɵtemplate(1, Message_Conditional_5_div_0_span_1_Template, 1, 4, "span", 8);
    ɵɵelementEnd();
  }
  if (rf & 2) {
    const ctx_r0 = ɵɵnextContext(2);
    ɵɵadvance();
    ɵɵproperty("ngIf", !ctx_r0.escape);
  }
}
function Message_Conditional_5_ng_template_1_span_0_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵelementStart(0, "span", 7);
    ɵɵtext(1);
    ɵɵelementEnd();
  }
  if (rf & 2) {
    const ctx_r0 = ɵɵnextContext(3);
    ɵɵproperty("pBind", ctx_r0.ptm("text"))("ngClass", ctx_r0.cx("text"));
    ɵɵattribute("data-p", ctx_r0.dataP);
    ɵɵadvance();
    ɵɵtextInterpolate(ctx_r0.text);
  }
}
function Message_Conditional_5_ng_template_1_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵtemplate(0, Message_Conditional_5_ng_template_1_span_0_Template, 2, 4, "span", 10);
  }
  if (rf & 2) {
    const ctx_r0 = ɵɵnextContext(2);
    ɵɵproperty("ngIf", ctx_r0.escape && ctx_r0.text);
  }
}
function Message_Conditional_5_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵtemplate(0, Message_Conditional_5_div_0_Template, 2, 1, "div", 6)(1, Message_Conditional_5_ng_template_1_Template, 1, 1, "ng-template", null, 0, ɵɵtemplateRefExtractor);
    ɵɵelementStart(3, "span", 7);
    ɵɵprojection(4);
    ɵɵelementEnd();
  }
  if (rf & 2) {
    const escapeOut_r2 = ɵɵreference(2);
    const ctx_r0 = ɵɵnextContext();
    ɵɵproperty("ngIf", !ctx_r0.escape)("ngIfElse", escapeOut_r2);
    ɵɵadvance(3);
    ɵɵproperty("pBind", ctx_r0.ptm("text"))("ngClass", ctx_r0.cx("text"));
    ɵɵattribute("data-p", ctx_r0.dataP);
  }
}
function Message_Conditional_6_Conditional_1_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵelement(0, "i", 7);
  }
  if (rf & 2) {
    const ctx_r0 = ɵɵnextContext(2);
    ɵɵclassMap(ctx_r0.cn(ctx_r0.cx("closeIcon"), ctx_r0.closeIcon));
    ɵɵproperty("pBind", ctx_r0.ptm("closeIcon"))("ngClass", ctx_r0.closeIcon);
    ɵɵattribute("data-p", ctx_r0.dataP);
  }
}
function Message_Conditional_6_Conditional_2_ng_container_0_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵelementContainer(0);
  }
}
function Message_Conditional_6_Conditional_2_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵtemplate(0, Message_Conditional_6_Conditional_2_ng_container_0_Template, 1, 0, "ng-container", 4);
  }
  if (rf & 2) {
    const ctx_r0 = ɵɵnextContext(2);
    ɵɵproperty("ngTemplateOutlet", ctx_r0.closeIconTemplate || ctx_r0._closeIconTemplate);
  }
}
function Message_Conditional_6_Conditional_3_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵnamespaceSVG();
    ɵɵelement(0, "svg", 14);
  }
  if (rf & 2) {
    const ctx_r0 = ɵɵnextContext(2);
    ɵɵclassMap(ctx_r0.cx("closeIcon"));
    ɵɵproperty("pBind", ctx_r0.ptm("closeIcon"));
    ɵɵattribute("data-p", ctx_r0.dataP);
  }
}
function Message_Conditional_6_Template(rf, ctx) {
  if (rf & 1) {
    const _r3 = ɵɵgetCurrentView();
    ɵɵelementStart(0, "button", 11);
    ɵɵlistener("click", function Message_Conditional_6_Template_button_click_0_listener($event) {
      ɵɵrestoreView(_r3);
      const ctx_r0 = ɵɵnextContext();
      return ɵɵresetView(ctx_r0.close($event));
    });
    ɵɵconditionalCreate(1, Message_Conditional_6_Conditional_1_Template, 1, 5, "i", 12);
    ɵɵconditionalCreate(2, Message_Conditional_6_Conditional_2_Template, 1, 1, "ng-container");
    ɵɵconditionalCreate(3, Message_Conditional_6_Conditional_3_Template, 1, 4, ":svg:svg", 13);
    ɵɵelementEnd();
  }
  if (rf & 2) {
    const ctx_r0 = ɵɵnextContext();
    ɵɵclassMap(ctx_r0.cx("closeButton"));
    ɵɵproperty("pBind", ctx_r0.ptm("closeButton"));
    ɵɵattribute("aria-label", ctx_r0.closeAriaLabel)("data-p", ctx_r0.dataP);
    ɵɵadvance();
    ɵɵconditional(ctx_r0.closeIcon ? 1 : -1);
    ɵɵadvance();
    ɵɵconditional(ctx_r0.closeIconTemplate || ctx_r0._closeIconTemplate ? 2 : -1);
    ɵɵadvance();
    ɵɵconditional(!ctx_r0.closeIconTemplate && !ctx_r0._closeIconTemplate && !ctx_r0.closeIcon ? 3 : -1);
  }
}
var classes = {
  root: ({
    instance
  }) => ["p-message p-component p-message-" + instance.severity, instance.variant && "p-message-" + instance.variant, {
    "p-message-sm": instance.size === "small",
    "p-message-lg": instance.size === "large"
  }],
  contentWrapper: "p-message-content-wrapper",
  content: "p-message-content",
  icon: "p-message-icon",
  text: "p-message-text",
  closeButton: "p-message-close-button",
  closeIcon: "p-message-close-icon"
};
var MessageStyle = class _MessageStyle extends BaseStyle {
  name = "message";
  style = style;
  classes = classes;
  static ɵfac = /* @__PURE__ */ (() => {
    let ɵMessageStyle_BaseFactory;
    return function MessageStyle_Factory(__ngFactoryType__) {
      return (ɵMessageStyle_BaseFactory || (ɵMessageStyle_BaseFactory = ɵɵgetInheritedFactory(_MessageStyle)))(__ngFactoryType__ || _MessageStyle);
    };
  })();
  static ɵprov = ɵɵdefineInjectable({
    token: _MessageStyle,
    factory: _MessageStyle.ɵfac
  });
};
(() => {
  (typeof ngDevMode === "undefined" || ngDevMode) && setClassMetadata(MessageStyle, [{
    type: Injectable
  }], null, null);
})();
var MessageClasses;
(function(MessageClasses2) {
  MessageClasses2["root"] = "p-message";
  MessageClasses2["content"] = "p-message-content";
  MessageClasses2["icon"] = "p-message-icon";
  MessageClasses2["text"] = "p-message-text";
  MessageClasses2["closeButton"] = "p-message-close-button";
  MessageClasses2["closeIcon"] = "p-message-close-icon";
})(MessageClasses || (MessageClasses = {}));
var MESSAGE_INSTANCE = new InjectionToken("MESSAGE_INSTANCE");
var Message = class _Message extends BaseComponent {
  componentName = "Message";
  _componentStyle = inject(MessageStyle);
  bindDirectiveInstance = inject(Bind, {
    self: true
  });
  $pcMessage = inject(MESSAGE_INSTANCE, {
    optional: true,
    skipSelf: true
  }) ?? void 0;
  onAfterViewChecked() {
    this.bindDirectiveInstance.setAttrs(this.ptms(["host", "root"]));
  }
  /**
   * Severity level of the message.
   * @defaultValue 'info'
   * @group Props
   */
  severity = "info";
  /**
   * Text content.
   * @deprecated since v20.0.0. Use content projection instead '<p-message>Content</p-message>'.
   * @group Props
   */
  text;
  /**
   * Whether displaying messages would be escaped or not.
   * @deprecated since v20.0.0. Use content projection instead '<p-message>Content</p-message>'.
   * @group Props
   */
  escape = true;
  /**
   * Inline style of the component.
   * @group Props
   */
  style;
  /**
   * Style class of the component.
   * @group Props
   */
  styleClass;
  /**
   * Whether the message can be closed manually using the close icon.
   * @group Props
   * @defaultValue false
   */
  closable = false;
  /**
   * Icon to display in the message.
   * @group Props
   * @defaultValue undefined
   */
  icon;
  /**
   * Icon to display in the message close button.
   * @group Props
   * @defaultValue undefined
   */
  closeIcon;
  /**
   * Delay in milliseconds to close the message automatically.
   * @defaultValue undefined
   */
  life;
  /**
   * Transition options of the show animation.
   * @defaultValue '300ms ease-out'
   * @group Props
   * @deprecated since v21.0.0, use `motionOptions` instead.
   */
  showTransitionOptions = "300ms ease-out";
  /**
   * Transition options of the hide animation.
   * @defaultValue '200ms cubic-bezier(0.86, 0, 0.07, 1)'
   * @group Props
   * @deprecated since v21.0.0, use `motionOptions` instead.
   */
  hideTransitionOptions = "200ms cubic-bezier(0.86, 0, 0.07, 1)";
  /**
   * Defines the size of the component.
   * @group Props
   */
  size;
  /**
   * Specifies the input variant of the component.
   * @group Props
   */
  variant;
  /**
   * The motion options.
   * @group Props
   */
  motionOptions = input(void 0, ...ngDevMode ? [{
    debugName: "motionOptions"
  }] : (
    /* istanbul ignore next */
    []
  ));
  computedMotionOptions = computed(() => {
    return __spreadValues(__spreadValues({}, this.ptm("motion")), this.motionOptions());
  }, ...ngDevMode ? [{
    debugName: "computedMotionOptions"
  }] : (
    /* istanbul ignore next */
    []
  ));
  /**
   * Emits when the message is closed.
   * @param {{ originalEvent: Event }} event - The event object containing the original event.
   * @group Emits
   */
  onClose = new EventEmitter();
  get closeAriaLabel() {
    return this.config.translation.aria ? this.config.translation.aria.close : void 0;
  }
  visible = signal(true, ...ngDevMode ? [{
    debugName: "visible"
  }] : (
    /* istanbul ignore next */
    []
  ));
  /**
   * Custom template of the message container.
   * @param {MessageContainerTemplateContext} context - container context.
   * @see {@link MessageContainerTemplateContext}
   * @group Templates
   */
  containerTemplate;
  /**
   * Custom template of the message icon.
   * @group Templates
   */
  iconTemplate;
  /**
   * Custom template of the close icon.
   * @group Templates
   */
  closeIconTemplate;
  templates;
  _containerTemplate;
  _iconTemplate;
  _closeIconTemplate;
  closeCallback = (event) => {
    this.close(event);
  };
  onInit() {
    if (this.life) {
      setTimeout(() => {
        this.visible.set(false);
      }, this.life);
    }
  }
  onAfterContentInit() {
    this.templates?.forEach((item) => {
      switch (item.getType()) {
        case "container":
          this._containerTemplate = item.template;
          break;
        case "icon":
          this._iconTemplate = item.template;
          break;
        case "closeicon":
          this._closeIconTemplate = item.template;
          break;
      }
    });
  }
  /**
   * Closes the message.
   * @param {Event} event - Browser event.
   * @group Method
   */
  close(event) {
    this.visible.set(false);
    this.onClose.emit({
      originalEvent: event
    });
  }
  get dataP() {
    return this.cn({
      outlined: this.variant === "outlined",
      simple: this.variant === "simple",
      [this.severity]: this.severity,
      [this.size]: this.size
    });
  }
  static ɵfac = /* @__PURE__ */ (() => {
    let ɵMessage_BaseFactory;
    return function Message_Factory(__ngFactoryType__) {
      return (ɵMessage_BaseFactory || (ɵMessage_BaseFactory = ɵɵgetInheritedFactory(_Message)))(__ngFactoryType__ || _Message);
    };
  })();
  static ɵcmp = ɵɵdefineComponent({
    type: _Message,
    selectors: [["p-message"]],
    contentQueries: function Message_ContentQueries(rf, ctx, dirIndex) {
      if (rf & 1) {
        ɵɵcontentQuery(dirIndex, _c0, 4)(dirIndex, _c1, 4)(dirIndex, _c2, 4)(dirIndex, PrimeTemplate, 4);
      }
      if (rf & 2) {
        let _t;
        ɵɵqueryRefresh(_t = ɵɵloadQuery()) && (ctx.containerTemplate = _t.first);
        ɵɵqueryRefresh(_t = ɵɵloadQuery()) && (ctx.iconTemplate = _t.first);
        ɵɵqueryRefresh(_t = ɵɵloadQuery()) && (ctx.closeIconTemplate = _t.first);
        ɵɵqueryRefresh(_t = ɵɵloadQuery()) && (ctx.templates = _t);
      }
    },
    hostAttrs: ["role", "alert", "aria-live", "polite"],
    hostVars: 5,
    hostBindings: function Message_HostBindings(rf, ctx) {
      if (rf & 1) {
        ɵɵanimateEnter(function Message_HostBindings_animateenter_cb() {
          return "p-message-enter-active";
        });
        ɵɵanimateLeave(function Message_HostBindings_animateleave_cb() {
          return "p-message-leave-active";
        });
      }
      if (rf & 2) {
        ɵɵattribute("data-p", ctx.dataP);
        ɵɵclassMap(ctx.cn(ctx.cx("root"), ctx.styleClass));
        ɵɵclassProp("p-message-leave-active", !ctx.visible());
      }
    },
    inputs: {
      severity: "severity",
      text: "text",
      escape: [2, "escape", "escape", booleanAttribute],
      style: "style",
      styleClass: "styleClass",
      closable: [2, "closable", "closable", booleanAttribute],
      icon: "icon",
      closeIcon: "closeIcon",
      life: "life",
      showTransitionOptions: "showTransitionOptions",
      hideTransitionOptions: "hideTransitionOptions",
      size: "size",
      variant: "variant",
      motionOptions: [1, "motionOptions"]
    },
    outputs: {
      onClose: "onClose"
    },
    features: [ɵɵProvidersFeature([MessageStyle, {
      provide: MESSAGE_INSTANCE,
      useExisting: _Message
    }, {
      provide: PARENT_INSTANCE,
      useExisting: _Message
    }]), ɵɵHostDirectivesFeature([Bind]), ɵɵInheritDefinitionFeature],
    ngContentSelectors: _c3,
    decls: 7,
    vars: 12,
    consts: [["escapeOut", ""], [3, "pBind"], [3, "pBind", "class"], ["pRipple", "", "type", "button", 3, "pBind", "class"], [4, "ngTemplateOutlet"], [4, "ngTemplateOutlet", "ngTemplateOutletContext"], [4, "ngIf", "ngIfElse"], [3, "pBind", "ngClass"], [3, "pBind", "ngClass", "innerHTML", 4, "ngIf"], [3, "pBind", "ngClass", "innerHTML"], [3, "pBind", "ngClass", 4, "ngIf"], ["pRipple", "", "type", "button", 3, "click", "pBind"], [3, "pBind", "class", "ngClass"], ["data-p-icon", "times", 3, "pBind", "class"], ["data-p-icon", "times", 3, "pBind"]],
    template: function Message_Template(rf, ctx) {
      if (rf & 1) {
        ɵɵprojectionDef();
        ɵɵelementStart(0, "div", 1)(1, "div", 1);
        ɵɵconditionalCreate(2, Message_Conditional_2_Template, 1, 1, "ng-container");
        ɵɵconditionalCreate(3, Message_Conditional_3_Template, 1, 4, "i", 2);
        ɵɵconditionalCreate(4, Message_Conditional_4_Template, 1, 4, "ng-container")(5, Message_Conditional_5_Template, 5, 5);
        ɵɵconditionalCreate(6, Message_Conditional_6_Template, 4, 8, "button", 3);
        ɵɵelementEnd()();
      }
      if (rf & 2) {
        ɵɵclassMap(ctx.cx("contentWrapper"));
        ɵɵproperty("pBind", ctx.ptm("contentWrapper"));
        ɵɵattribute("data-p", ctx.dataP);
        ɵɵadvance();
        ɵɵclassMap(ctx.cx("content"));
        ɵɵproperty("pBind", ctx.ptm("content"));
        ɵɵattribute("data-p", ctx.dataP);
        ɵɵadvance();
        ɵɵconditional(ctx.iconTemplate || ctx._iconTemplate ? 2 : -1);
        ɵɵadvance();
        ɵɵconditional(ctx.icon ? 3 : -1);
        ɵɵadvance();
        ɵɵconditional(ctx.containerTemplate || ctx._containerTemplate ? 4 : 5);
        ɵɵadvance(2);
        ɵɵconditional(ctx.closable ? 6 : -1);
      }
    },
    dependencies: [CommonModule, NgClass, NgIf, NgTemplateOutlet, TimesIcon, Ripple, SharedModule, Bind, MotionModule],
    encapsulation: 2,
    changeDetection: 0
  });
};
(() => {
  (typeof ngDevMode === "undefined" || ngDevMode) && setClassMetadata(Message, [{
    type: Component,
    args: [{
      selector: "p-message",
      standalone: true,
      imports: [CommonModule, TimesIcon, Ripple, SharedModule, Bind, MotionModule],
      template: `
        <div [pBind]="ptm('contentWrapper')" [class]="cx('contentWrapper')" [attr.data-p]="dataP">
            <div [pBind]="ptm('content')" [class]="cx('content')" [attr.data-p]="dataP">
                @if (iconTemplate || _iconTemplate) {
                    <ng-container *ngTemplateOutlet="iconTemplate || _iconTemplate"></ng-container>
                }
                @if (icon) {
                    <i [pBind]="ptm('icon')" [class]="cn(cx('icon'), icon)" [attr.data-p]="dataP"></i>
                }

                @if (containerTemplate || _containerTemplate) {
                    <ng-container *ngTemplateOutlet="containerTemplate || _containerTemplate; context: { closeCallback: closeCallback }"></ng-container>
                } @else {
                    <div *ngIf="!escape; else escapeOut">
                        <span [pBind]="ptm('text')" *ngIf="!escape" [ngClass]="cx('text')" [innerHTML]="text" [attr.data-p]="dataP"></span>
                    </div>

                    <ng-template #escapeOut>
                        <span [pBind]="ptm('text')" *ngIf="escape && text" [ngClass]="cx('text')" [attr.data-p]="dataP">{{ text }}</span>
                    </ng-template>

                    <span [pBind]="ptm('text')" [ngClass]="cx('text')" [attr.data-p]="dataP">
                        <ng-content></ng-content>
                    </span>
                }
                @if (closable) {
                    <button [pBind]="ptm('closeButton')" pRipple type="button" [class]="cx('closeButton')" (click)="close($event)" [attr.aria-label]="closeAriaLabel" [attr.data-p]="dataP">
                        @if (closeIcon) {
                            <i [pBind]="ptm('closeIcon')" [class]="cn(cx('closeIcon'), closeIcon)" [ngClass]="closeIcon" [attr.data-p]="dataP"></i>
                        }
                        @if (closeIconTemplate || _closeIconTemplate) {
                            <ng-container *ngTemplateOutlet="closeIconTemplate || _closeIconTemplate"></ng-container>
                        }
                        @if (!closeIconTemplate && !_closeIconTemplate && !closeIcon) {
                            <svg [pBind]="ptm('closeIcon')" data-p-icon="times" [class]="cx('closeIcon')" [attr.data-p]="dataP" />
                        }
                    </button>
                }
            </div>
        </div>
    `,
      changeDetection: ChangeDetectionStrategy.OnPush,
      encapsulation: ViewEncapsulation.None,
      providers: [MessageStyle, {
        provide: MESSAGE_INSTANCE,
        useExisting: Message
      }, {
        provide: PARENT_INSTANCE,
        useExisting: Message
      }],
      hostDirectives: [Bind],
      host: {
        "[attr.data-p]": "dataP",
        role: "alert",
        "aria-live": "polite",
        "[class]": 'cn(cx("root"), styleClass)',
        "[animate.enter]": '"p-message-enter-active"',
        "[animate.leave]": '"p-message-leave-active"',
        "[class.p-message-leave-active]": "!visible()"
      }
    }]
  }], null, {
    severity: [{
      type: Input
    }],
    text: [{
      type: Input
    }],
    escape: [{
      type: Input,
      args: [{
        transform: booleanAttribute
      }]
    }],
    style: [{
      type: Input
    }],
    styleClass: [{
      type: Input
    }],
    closable: [{
      type: Input,
      args: [{
        transform: booleanAttribute
      }]
    }],
    icon: [{
      type: Input
    }],
    closeIcon: [{
      type: Input
    }],
    life: [{
      type: Input
    }],
    showTransitionOptions: [{
      type: Input
    }],
    hideTransitionOptions: [{
      type: Input
    }],
    size: [{
      type: Input
    }],
    variant: [{
      type: Input
    }],
    motionOptions: [{
      type: Input,
      args: [{
        isSignal: true,
        alias: "motionOptions",
        required: false
      }]
    }],
    onClose: [{
      type: Output
    }],
    containerTemplate: [{
      type: ContentChild,
      args: ["container", {
        descendants: false
      }]
    }],
    iconTemplate: [{
      type: ContentChild,
      args: ["icon", {
        descendants: false
      }]
    }],
    closeIconTemplate: [{
      type: ContentChild,
      args: ["closeicon", {
        descendants: false
      }]
    }],
    templates: [{
      type: ContentChildren,
      args: [PrimeTemplate]
    }]
  });
})();
var MessageModule = class _MessageModule {
  static ɵfac = function MessageModule_Factory(__ngFactoryType__) {
    return new (__ngFactoryType__ || _MessageModule)();
  };
  static ɵmod = ɵɵdefineNgModule({
    type: _MessageModule,
    imports: [Message, SharedModule],
    exports: [Message, SharedModule]
  });
  static ɵinj = ɵɵdefineInjector({
    imports: [Message, SharedModule, SharedModule]
  });
};
(() => {
  (typeof ngDevMode === "undefined" || ngDevMode) && setClassMetadata(MessageModule, [{
    type: NgModule,
    args: [{
      imports: [Message, SharedModule],
      exports: [Message, SharedModule]
    }]
  }], null, null);
})();

// node_modules/@primeuix/styles/dist/progressbar/index.mjs
var style2 = "\n    .p-progressbar {\n        display: block;\n        position: relative;\n        overflow: hidden;\n        height: dt('progressbar.height');\n        background: dt('progressbar.background');\n        border-radius: dt('progressbar.border.radius');\n    }\n\n    .p-progressbar-value {\n        margin: 0;\n        background: dt('progressbar.value.background');\n    }\n\n    .p-progressbar-label {\n        color: dt('progressbar.label.color');\n        font-size: dt('progressbar.label.font.size');\n        font-weight: dt('progressbar.label.font.weight');\n    }\n\n    .p-progressbar-determinate .p-progressbar-value {\n        height: 100%;\n        width: 0%;\n        position: absolute;\n        display: none;\n        display: flex;\n        align-items: center;\n        justify-content: center;\n        overflow: hidden;\n        transition: width 1s ease-in-out;\n    }\n\n    .p-progressbar-determinate .p-progressbar-label {\n        display: inline-flex;\n    }\n\n    .p-progressbar-indeterminate .p-progressbar-value::before {\n        content: '';\n        position: absolute;\n        background: inherit;\n        inset-block-start: 0;\n        inset-inline-start: 0;\n        inset-block-end: 0;\n        will-change: inset-inline-start, inset-inline-end;\n        animation: p-progressbar-indeterminate-anim 2.1s cubic-bezier(0.65, 0.815, 0.735, 0.395) infinite;\n    }\n\n    .p-progressbar-indeterminate .p-progressbar-value::after {\n        content: '';\n        position: absolute;\n        background: inherit;\n        inset-block-start: 0;\n        inset-inline-start: 0;\n        inset-block-end: 0;\n        will-change: inset-inline-start, inset-inline-end;\n        animation: p-progressbar-indeterminate-anim-short 2.1s cubic-bezier(0.165, 0.84, 0.44, 1) infinite;\n        animation-delay: 1.15s;\n    }\n\n    @keyframes p-progressbar-indeterminate-anim {\n        0% {\n            inset-inline-start: -35%;\n            inset-inline-end: 100%;\n        }\n        60% {\n            inset-inline-start: 100%;\n            inset-inline-end: -90%;\n        }\n        100% {\n            inset-inline-start: 100%;\n            inset-inline-end: -90%;\n        }\n    }\n    @-webkit-keyframes p-progressbar-indeterminate-anim {\n        0% {\n            inset-inline-start: -35%;\n            inset-inline-end: 100%;\n        }\n        60% {\n            inset-inline-start: 100%;\n            inset-inline-end: -90%;\n        }\n        100% {\n            inset-inline-start: 100%;\n            inset-inline-end: -90%;\n        }\n    }\n\n    @keyframes p-progressbar-indeterminate-anim-short {\n        0% {\n            inset-inline-start: -200%;\n            inset-inline-end: 100%;\n        }\n        60% {\n            inset-inline-start: 107%;\n            inset-inline-end: -8%;\n        }\n        100% {\n            inset-inline-start: 107%;\n            inset-inline-end: -8%;\n        }\n    }\n    @-webkit-keyframes p-progressbar-indeterminate-anim-short {\n        0% {\n            inset-inline-start: -200%;\n            inset-inline-end: 100%;\n        }\n        60% {\n            inset-inline-start: 107%;\n            inset-inline-end: -8%;\n        }\n        100% {\n            inset-inline-start: 107%;\n            inset-inline-end: -8%;\n        }\n    }\n";

// node_modules/primeng/fesm2022/primeng-progressbar.mjs
var _c02 = ["content"];
var _c12 = (a0) => ({
  $implicit: a0
});
function ProgressBar_div_0_div_2_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵelementStart(0, "div");
    ɵɵtext(1);
    ɵɵelementEnd();
  }
  if (rf & 2) {
    const ctx_r0 = ɵɵnextContext(2);
    ɵɵstyleProp("display", ctx_r0.value != null && ctx_r0.value !== 0 ? "flex" : "none");
    ɵɵadvance();
    ɵɵtextInterpolate2("", ctx_r0.value, "", ctx_r0.unit);
  }
}
function ProgressBar_div_0_ng_container_3_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵelementContainer(0);
  }
}
function ProgressBar_div_0_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵelementStart(0, "div", 2)(1, "div", 2);
    ɵɵtemplate(2, ProgressBar_div_0_div_2_Template, 2, 4, "div", 3)(3, ProgressBar_div_0_ng_container_3_Template, 1, 0, "ng-container", 4);
    ɵɵelementEnd()();
  }
  if (rf & 2) {
    const ctx_r0 = ɵɵnextContext();
    ɵɵclassMap(ctx_r0.cn(ctx_r0.cx("value"), ctx_r0.valueStyleClass));
    ɵɵstyleProp("width", ctx_r0.value + "%")("display", "flex")("background", ctx_r0.color);
    ɵɵproperty("pBind", ctx_r0.ptm("value"));
    ɵɵattribute("data-p", ctx_r0.dataP);
    ɵɵadvance();
    ɵɵclassMap(ctx_r0.cx("label"));
    ɵɵproperty("pBind", ctx_r0.ptm("label"));
    ɵɵattribute("data-p", ctx_r0.dataP);
    ɵɵadvance();
    ɵɵproperty("ngIf", ctx_r0.showValue && !ctx_r0.contentTemplate && !ctx_r0._contentTemplate);
    ɵɵadvance();
    ɵɵproperty("ngTemplateOutlet", ctx_r0.contentTemplate || ctx_r0._contentTemplate)("ngTemplateOutletContext", ɵɵpureFunction1(17, _c12, ctx_r0.value));
  }
}
function ProgressBar_div_1_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵelement(0, "div", 2);
  }
  if (rf & 2) {
    const ctx_r0 = ɵɵnextContext();
    ɵɵclassMap(ctx_r0.cn(ctx_r0.cx("value"), ctx_r0.valueStyleClass));
    ɵɵstyleProp("background", ctx_r0.color);
    ɵɵproperty("pBind", ctx_r0.ptm("value"));
    ɵɵattribute("data-p", ctx_r0.dataP);
  }
}
var classes2 = {
  root: ({
    instance
  }) => ["p-progressbar p-component", {
    "p-progressbar-determinate": instance.mode == "determinate",
    "p-progressbar-indeterminate": instance.mode == "indeterminate"
  }],
  value: "p-progressbar-value",
  label: "p-progressbar-label"
};
var ProgressBarStyle = class _ProgressBarStyle extends BaseStyle {
  name = "progressbar";
  style = style2;
  classes = classes2;
  static ɵfac = /* @__PURE__ */ (() => {
    let ɵProgressBarStyle_BaseFactory;
    return function ProgressBarStyle_Factory(__ngFactoryType__) {
      return (ɵProgressBarStyle_BaseFactory || (ɵProgressBarStyle_BaseFactory = ɵɵgetInheritedFactory(_ProgressBarStyle)))(__ngFactoryType__ || _ProgressBarStyle);
    };
  })();
  static ɵprov = ɵɵdefineInjectable({
    token: _ProgressBarStyle,
    factory: _ProgressBarStyle.ɵfac
  });
};
(() => {
  (typeof ngDevMode === "undefined" || ngDevMode) && setClassMetadata(ProgressBarStyle, [{
    type: Injectable
  }], null, null);
})();
var ProgressBarClasses;
(function(ProgressBarClasses2) {
  ProgressBarClasses2["root"] = "p-progressbar";
  ProgressBarClasses2["value"] = "p-progressbar-value";
  ProgressBarClasses2["label"] = "p-progressbar-label";
})(ProgressBarClasses || (ProgressBarClasses = {}));
var PROGRESSBAR_INSTANCE = new InjectionToken("PROGRESSBAR_INSTANCE");
var ProgressBar = class _ProgressBar extends BaseComponent {
  componentName = "ProgressBar";
  $pcProgressBar = inject(PROGRESSBAR_INSTANCE, {
    optional: true,
    skipSelf: true
  }) ?? void 0;
  bindDirectiveInstance = inject(Bind, {
    self: true
  });
  /**
   * Current value of the progress.
   * @group Props
   */
  value;
  /**
   * Whether to display the progress bar value.
   * @group Props
   */
  showValue = true;
  /**
   * Style class of the element.
   * @deprecated since v20.0.0, use `class` instead.
   * @group Props
   */
  styleClass;
  /**
   * Style class of the value element.
   * @group Props
   */
  valueStyleClass;
  /**
   * Unit sign appended to the value.
   * @group Props
   */
  unit = "%";
  /**
   * Defines the mode of the progress
   * @defaultValue 'determinate'
   * @group Props
   */
  mode = "determinate";
  /**
   * Color for the background of the progress.
   * @group Props
   */
  color;
  /**
   * Template of the content.
   * @param {ProgressBarContentTemplateContext} context - content context.
   * @see {@link ProgressBarContentTemplateContext}
   * @group Templates
   */
  contentTemplate;
  onAfterViewChecked() {
    this.bindDirectiveInstance.setAttrs(this.ptms(["host", "root"]));
  }
  _componentStyle = inject(ProgressBarStyle);
  templates;
  _contentTemplate;
  onAfterContentInit() {
    this.templates?.forEach((item) => {
      switch (item.getType()) {
        case "content":
          this._contentTemplate = item.template;
          break;
        default:
          this._contentTemplate = item.template;
      }
    });
  }
  get dataP() {
    return this.cn({
      determinate: this.mode === "determinate",
      indeterminate: this.mode === "indeterminate"
    });
  }
  static ɵfac = /* @__PURE__ */ (() => {
    let ɵProgressBar_BaseFactory;
    return function ProgressBar_Factory(__ngFactoryType__) {
      return (ɵProgressBar_BaseFactory || (ɵProgressBar_BaseFactory = ɵɵgetInheritedFactory(_ProgressBar)))(__ngFactoryType__ || _ProgressBar);
    };
  })();
  static ɵcmp = ɵɵdefineComponent({
    type: _ProgressBar,
    selectors: [["p-progressBar"], ["p-progressbar"], ["p-progress-bar"]],
    contentQueries: function ProgressBar_ContentQueries(rf, ctx, dirIndex) {
      if (rf & 1) {
        ɵɵcontentQuery(dirIndex, _c02, 4)(dirIndex, PrimeTemplate, 4);
      }
      if (rf & 2) {
        let _t;
        ɵɵqueryRefresh(_t = ɵɵloadQuery()) && (ctx.contentTemplate = _t.first);
        ɵɵqueryRefresh(_t = ɵɵloadQuery()) && (ctx.templates = _t);
      }
    },
    hostAttrs: ["role", "progressbar"],
    hostVars: 7,
    hostBindings: function ProgressBar_HostBindings(rf, ctx) {
      if (rf & 2) {
        ɵɵattribute("aria-valuemin", 0)("aria-valuenow", ctx.value)("aria-valuemax", 100)("aria-level", ctx.value + ctx.unit)("data-p", ctx.dataP);
        ɵɵclassMap(ctx.cn(ctx.cx("root"), ctx.styleClass));
      }
    },
    inputs: {
      value: [2, "value", "value", numberAttribute],
      showValue: [2, "showValue", "showValue", booleanAttribute],
      styleClass: "styleClass",
      valueStyleClass: "valueStyleClass",
      unit: "unit",
      mode: "mode",
      color: "color"
    },
    features: [ɵɵProvidersFeature([ProgressBarStyle, {
      provide: PROGRESSBAR_INSTANCE,
      useExisting: _ProgressBar
    }, {
      provide: PARENT_INSTANCE,
      useExisting: _ProgressBar
    }]), ɵɵHostDirectivesFeature([Bind]), ɵɵInheritDefinitionFeature],
    decls: 2,
    vars: 2,
    consts: [[3, "class", "pBind", "width", "display", "background", 4, "ngIf"], [3, "class", "pBind", "background", 4, "ngIf"], [3, "pBind"], [3, "display", 4, "ngIf"], [4, "ngTemplateOutlet", "ngTemplateOutletContext"]],
    template: function ProgressBar_Template(rf, ctx) {
      if (rf & 1) {
        ɵɵtemplate(0, ProgressBar_div_0_Template, 4, 19, "div", 0)(1, ProgressBar_div_1_Template, 1, 6, "div", 1);
      }
      if (rf & 2) {
        ɵɵproperty("ngIf", ctx.mode === "determinate");
        ɵɵadvance();
        ɵɵproperty("ngIf", ctx.mode === "indeterminate");
      }
    },
    dependencies: [CommonModule, NgIf, NgTemplateOutlet, SharedModule, Bind],
    encapsulation: 2,
    changeDetection: 0
  });
};
(() => {
  (typeof ngDevMode === "undefined" || ngDevMode) && setClassMetadata(ProgressBar, [{
    type: Component,
    args: [{
      selector: "p-progressBar, p-progressbar, p-progress-bar",
      standalone: true,
      imports: [CommonModule, SharedModule, Bind],
      template: `
        <div *ngIf="mode === 'determinate'" [class]="cn(cx('value'), valueStyleClass)" [pBind]="ptm('value')" [style.width]="value + '%'" [style.display]="'flex'" [style.background]="color" [attr.data-p]="dataP">
            <div [class]="cx('label')" [pBind]="ptm('label')" [attr.data-p]="dataP">
                <div *ngIf="showValue && !contentTemplate && !_contentTemplate" [style.display]="value != null && value !== 0 ? 'flex' : 'none'">{{ value }}{{ unit }}</div>
                <ng-container *ngTemplateOutlet="contentTemplate || _contentTemplate; context: { $implicit: value }"></ng-container>
            </div>
        </div>
        <div *ngIf="mode === 'indeterminate'" [class]="cn(cx('value'), valueStyleClass)" [pBind]="ptm('value')" [style.background]="color" [attr.data-p]="dataP"></div>
    `,
      changeDetection: ChangeDetectionStrategy.OnPush,
      encapsulation: ViewEncapsulation.None,
      providers: [ProgressBarStyle, {
        provide: PROGRESSBAR_INSTANCE,
        useExisting: ProgressBar
      }, {
        provide: PARENT_INSTANCE,
        useExisting: ProgressBar
      }],
      host: {
        role: "progressbar",
        "[attr.aria-valuemin]": "0",
        "[attr.aria-valuenow]": "value",
        "[attr.aria-valuemax]": "100",
        "[attr.aria-level]": "value + unit",
        "[class]": "cn(cx('root'), styleClass)",
        "[attr.data-p]": "dataP"
      },
      hostDirectives: [Bind]
    }]
  }], null, {
    value: [{
      type: Input,
      args: [{
        transform: numberAttribute
      }]
    }],
    showValue: [{
      type: Input,
      args: [{
        transform: booleanAttribute
      }]
    }],
    styleClass: [{
      type: Input
    }],
    valueStyleClass: [{
      type: Input
    }],
    unit: [{
      type: Input
    }],
    mode: [{
      type: Input
    }],
    color: [{
      type: Input
    }],
    contentTemplate: [{
      type: ContentChild,
      args: ["content", {
        descendants: false
      }]
    }],
    templates: [{
      type: ContentChildren,
      args: [PrimeTemplate]
    }]
  });
})();
var ProgressBarModule = class _ProgressBarModule {
  static ɵfac = function ProgressBarModule_Factory(__ngFactoryType__) {
    return new (__ngFactoryType__ || _ProgressBarModule)();
  };
  static ɵmod = ɵɵdefineNgModule({
    type: _ProgressBarModule,
    imports: [ProgressBar, SharedModule],
    exports: [ProgressBar, SharedModule]
  });
  static ɵinj = ɵɵdefineInjector({
    imports: [ProgressBar, SharedModule, SharedModule]
  });
};
(() => {
  (typeof ngDevMode === "undefined" || ngDevMode) && setClassMetadata(ProgressBarModule, [{
    type: NgModule,
    args: [{
      imports: [ProgressBar, SharedModule],
      exports: [ProgressBar, SharedModule]
    }]
  }], null, null);
})();

// node_modules/@primeuix/styles/dist/fileupload/index.mjs
var style3 = "\n    .p-fileupload input[type='file'] {\n        display: none;\n    }\n\n    .p-fileupload-advanced {\n        border: 1px solid dt('fileupload.border.color');\n        border-radius: dt('fileupload.border.radius');\n        background: dt('fileupload.background');\n        color: dt('fileupload.color');\n    }\n\n    .p-fileupload-header {\n        display: flex;\n        align-items: center;\n        padding: dt('fileupload.header.padding');\n        background: dt('fileupload.header.background');\n        color: dt('fileupload.header.color');\n        border-style: solid;\n        border-width: dt('fileupload.header.border.width');\n        border-color: dt('fileupload.header.border.color');\n        border-radius: dt('fileupload.header.border.radius');\n        gap: dt('fileupload.header.gap');\n    }\n\n    .p-fileupload-content {\n        border: 1px solid transparent;\n        display: flex;\n        flex-direction: column;\n        gap: dt('fileupload.content.gap');\n        transition: border-color dt('fileupload.transition.duration');\n        padding: dt('fileupload.content.padding');\n    }\n\n    .p-fileupload-content .p-progressbar {\n        width: 100%;\n        height: dt('fileupload.progressbar.height');\n    }\n\n    .p-fileupload-file-list {\n        display: flex;\n        flex-direction: column;\n        gap: dt('fileupload.filelist.gap');\n    }\n\n    .p-fileupload-file {\n        display: flex;\n        flex-wrap: wrap;\n        align-items: center;\n        padding: dt('fileupload.file.padding');\n        border-block-end: 1px solid dt('fileupload.file.border.color');\n        gap: dt('fileupload.file.gap');\n    }\n\n    .p-fileupload-file:last-child {\n        border-block-end: 0;\n    }\n\n    .p-fileupload-file-info {\n        display: flex;\n        flex-direction: column;\n        gap: dt('fileupload.file.info.gap');\n    }\n\n    .p-fileupload-file-thumbnail {\n        flex-shrink: 0;\n    }\n\n    .p-fileupload-file-actions {\n        margin-inline-start: auto;\n    }\n\n    .p-fileupload-highlight {\n        border: 1px dashed dt('fileupload.content.highlight.border.color');\n    }\n\n    .p-fileupload-basic .p-message {\n        margin-block-end: dt('fileupload.basic.gap');\n    }\n\n    .p-fileupload-basic-content {\n        display: flex;\n        flex-wrap: wrap;\n        align-items: center;\n        gap: dt('fileupload.basic.gap');\n    }\n";

// node_modules/primeng/fesm2022/primeng-fileupload.mjs
var _c03 = ["pFileContent", ""];
var _c13 = (a0, a1, a2) => ({
  class: a0,
  file: a1,
  index: a2
});
var _forTrack0 = ($index, $item) => ($item == null ? null : $item.name) + "-" + $index;
function FileContent_For_1_ng_template_10_Conditional_0_0_ng_template_0_Template(rf, ctx) {
}
function FileContent_For_1_ng_template_10_Conditional_0_0_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵtemplate(0, FileContent_For_1_ng_template_10_Conditional_0_0_ng_template_0_Template, 0, 0, "ng-template");
  }
}
function FileContent_For_1_ng_template_10_Conditional_0_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵtemplate(0, FileContent_For_1_ng_template_10_Conditional_0_0_Template, 1, 0, null, 7);
  }
  if (rf & 2) {
    const iconClass_r4 = ɵɵnextContext().class;
    const ctx_r4 = ɵɵnextContext();
    const file_r6 = ctx_r4.$implicit;
    const ɵ$index_1_r2 = ctx_r4.$index;
    const ctx_r2 = ɵɵnextContext();
    ɵɵproperty("ngTemplateOutlet", ctx_r2.fileRemoveIconTemplate())("ngTemplateOutletContext", ɵɵpureFunction3(2, _c13, iconClass_r4, file_r6, ɵ$index_1_r2));
  }
}
function FileContent_For_1_ng_template_10_Conditional_1_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵnamespaceSVG();
    ɵɵelement(0, "svg", 8);
  }
  if (rf & 2) {
    const iconClass_r4 = ɵɵnextContext().class;
    ɵɵclassMap(iconClass_r4);
    ɵɵattribute("aria-hidden", true);
  }
}
function FileContent_For_1_ng_template_10_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵconditionalCreate(0, FileContent_For_1_ng_template_10_Conditional_0_Template, 1, 6)(1, FileContent_For_1_ng_template_10_Conditional_1_Template, 1, 3, ":svg:svg", 6);
  }
  if (rf & 2) {
    const ctx_r2 = ɵɵnextContext(2);
    ɵɵconditional(ctx_r2.fileRemoveIconTemplate() ? 0 : 1);
  }
}
function FileContent_For_1_Template(rf, ctx) {
  if (rf & 1) {
    const _r1 = ɵɵgetCurrentView();
    ɵɵelementStart(0, "div", 2);
    ɵɵelement(1, "img", 3);
    ɵɵelementStart(2, "div", 2)(3, "div", 2);
    ɵɵtext(4);
    ɵɵelementEnd();
    ɵɵelementStart(5, "span", 2);
    ɵɵtext(6);
    ɵɵelementEnd()();
    ɵɵelement(7, "p-badge", 4);
    ɵɵelementStart(8, "div", 2)(9, "p-button", 5);
    ɵɵlistener("onClick", function FileContent_For_1_Template_p_button_onClick_9_listener($event) {
      const ɵ$index_1_r2 = ɵɵrestoreView(_r1).$index;
      const ctx_r2 = ɵɵnextContext();
      return ɵɵresetView(ctx_r2.onRemoveClick($event, ɵ$index_1_r2));
    });
    ɵɵtemplate(10, FileContent_For_1_ng_template_10_Template, 2, 1, "ng-template", null, 0, ɵɵtemplateRefExtractor);
    ɵɵelementEnd()()();
  }
  if (rf & 2) {
    const file_r6 = ctx.$implicit;
    const ctx_r2 = ɵɵnextContext();
    ɵɵclassMap(ctx_r2.cx("file"));
    ɵɵproperty("pBind", ctx_r2.$pcFileUpload.ptm("file"));
    ɵɵadvance();
    ɵɵclassMap(ctx_r2.cx("fileThumbnail"));
    ɵɵproperty("src", file_r6.objectURL, ɵɵsanitizeUrl)("width", ctx_r2.previewWidth())("pBind", ctx_r2.$pcFileUpload.ptm("fileThumbnail"));
    ɵɵattribute("alt", file_r6.name);
    ɵɵadvance();
    ɵɵclassMap(ctx_r2.cx("fileInfo"));
    ɵɵproperty("pBind", ctx_r2.$pcFileUpload.ptm("fileInfo"));
    ɵɵadvance();
    ɵɵclassMap(ctx_r2.cx("fileName"));
    ɵɵproperty("pBind", ctx_r2.$pcFileUpload.ptm("fileName"));
    ɵɵadvance();
    ɵɵtextInterpolate(file_r6.name);
    ɵɵadvance();
    ɵɵclassMap(ctx_r2.cx("fileSize"));
    ɵɵproperty("pBind", ctx_r2.$pcFileUpload.ptm("fileSize"));
    ɵɵadvance();
    ɵɵtextInterpolate(ctx_r2.formatSize(file_r6.size));
    ɵɵadvance();
    ɵɵclassMap(ctx_r2.cx("pcFileBadge"));
    ɵɵproperty("value", ctx_r2.badgeValue())("severity", ctx_r2.badgeSeverity())("pt", ctx_r2.$pcFileUpload.ptm("pcFileBadge"))("unstyled", ctx_r2.unstyled());
    ɵɵadvance();
    ɵɵclassMap(ctx_r2.cx("fileActions"));
    ɵɵproperty("pBind", ctx_r2.$pcFileUpload.ptm("fileActions"));
    ɵɵadvance();
    ɵɵproperty("styleClass", ctx_r2.cx("pcFileRemoveButton"))("pt", ctx_r2.$pcFileUpload.ptm("pcFileRemoveButton"))("unstyled", ctx_r2.unstyled());
  }
}
var _c22 = ["file"];
var _c32 = ["header"];
var _c42 = ["content"];
var _c5 = ["toolbar"];
var _c6 = ["chooseicon"];
var _c7 = ["filelabel"];
var _c8 = ["uploadicon"];
var _c9 = ["cancelicon"];
var _c10 = ["empty"];
var _c11 = ["advancedfileinput"];
var _c122 = ["basicfileinput"];
var _c132 = (a0, a1, a2, a3, a4) => ({
  $implicit: a0,
  uploadedFiles: a1,
  chooseCallback: a2,
  clearCallback: a3,
  uploadCallback: a4
});
var _c14 = (a0, a1, a2, a3, a4, a5, a6, a7) => ({
  $implicit: a0,
  uploadedFiles: a1,
  chooseCallback: a2,
  clearCallback: a3,
  removeUploadedFileCallback: a4,
  removeFileCallback: a5,
  progress: a6,
  messages: a7
});
var _c15 = (a0) => ({
  $implicit: a0
});
function FileUpload_div_0_ng_container_4_ng_template_4_span_0_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵelement(0, "span", 8);
  }
  if (rf & 2) {
    let tmp_9_0;
    const ctx_r1 = ɵɵnextContext(4);
    ɵɵclassMap(ctx_r1.chooseIcon);
    ɵɵproperty("pBind", (tmp_9_0 = ctx_r1.ptm("pcChooseButton")) == null ? null : tmp_9_0.icon);
    ɵɵattribute("aria-label", true);
  }
}
function FileUpload_div_0_ng_container_4_ng_template_4_ng_container_1__svg_svg_1_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵnamespaceSVG();
    ɵɵelement(0, "svg", 17);
  }
  if (rf & 2) {
    let tmp_9_0;
    const ctx_r1 = ɵɵnextContext(5);
    ɵɵproperty("pBind", (tmp_9_0 = ctx_r1.ptm("pcChooseButton")) == null ? null : tmp_9_0.icon);
    ɵɵattribute("aria-label", true);
  }
}
function FileUpload_div_0_ng_container_4_ng_template_4_ng_container_1_span_2_1_ng_template_0_Template(rf, ctx) {
}
function FileUpload_div_0_ng_container_4_ng_template_4_ng_container_1_span_2_1_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵtemplate(0, FileUpload_div_0_ng_container_4_ng_template_4_ng_container_1_span_2_1_ng_template_0_Template, 0, 0, "ng-template");
  }
}
function FileUpload_div_0_ng_container_4_ng_template_4_ng_container_1_span_2_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵelementStart(0, "span", 8);
    ɵɵtemplate(1, FileUpload_div_0_ng_container_4_ng_template_4_ng_container_1_span_2_1_Template, 1, 0, null, 11);
    ɵɵelementEnd();
  }
  if (rf & 2) {
    let tmp_9_0;
    const ctx_r1 = ɵɵnextContext(5);
    ɵɵproperty("pBind", (tmp_9_0 = ctx_r1.ptm("pcChooseButton")) == null ? null : tmp_9_0.icon);
    ɵɵattribute("aria-label", true);
    ɵɵadvance();
    ɵɵproperty("ngTemplateOutlet", ctx_r1.chooseIconTemplate || ctx_r1._chooseIconTemplate);
  }
}
function FileUpload_div_0_ng_container_4_ng_template_4_ng_container_1_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵelementContainerStart(0);
    ɵɵtemplate(1, FileUpload_div_0_ng_container_4_ng_template_4_ng_container_1__svg_svg_1_Template, 1, 2, "svg", 15)(2, FileUpload_div_0_ng_container_4_ng_template_4_ng_container_1_span_2_Template, 2, 3, "span", 16);
    ɵɵelementContainerEnd();
  }
  if (rf & 2) {
    const ctx_r1 = ɵɵnextContext(4);
    ɵɵadvance();
    ɵɵproperty("ngIf", !ctx_r1.chooseIconTemplate && !ctx_r1._chooseIconTemplate);
    ɵɵadvance();
    ɵɵproperty("ngIf", ctx_r1.chooseIconTemplate || ctx_r1._chooseIconTemplate);
  }
}
function FileUpload_div_0_ng_container_4_ng_template_4_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵtemplate(0, FileUpload_div_0_ng_container_4_ng_template_4_span_0_Template, 1, 4, "span", 5)(1, FileUpload_div_0_ng_container_4_ng_template_4_ng_container_1_Template, 3, 2, "ng-container", 9);
  }
  if (rf & 2) {
    const ctx_r1 = ɵɵnextContext(3);
    ɵɵproperty("ngIf", ctx_r1.chooseIcon);
    ɵɵadvance();
    ɵɵproperty("ngIf", !ctx_r1.chooseIcon);
  }
}
function FileUpload_div_0_ng_container_4_p_button_6_ng_template_1_span_0_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵelement(0, "span", 20);
  }
  if (rf & 2) {
    let tmp_11_0;
    const ctx_r1 = ɵɵnextContext(5);
    ɵɵproperty("ngClass", ctx_r1.uploadIcon)("pBind", (tmp_11_0 = ctx_r1.ptm("pcUploadButton")) == null ? null : tmp_11_0.icon);
    ɵɵattribute("aria-hidden", true);
  }
}
function FileUpload_div_0_ng_container_4_p_button_6_ng_template_1_ng_container_1__svg_svg_1_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵnamespaceSVG();
    ɵɵelement(0, "svg", 22);
  }
  if (rf & 2) {
    let tmp_11_0;
    const ctx_r1 = ɵɵnextContext(6);
    ɵɵproperty("pBind", (tmp_11_0 = ctx_r1.ptm("pcUploadButton")) == null ? null : tmp_11_0.icon);
  }
}
function FileUpload_div_0_ng_container_4_p_button_6_ng_template_1_ng_container_1_span_2_1_ng_template_0_Template(rf, ctx) {
}
function FileUpload_div_0_ng_container_4_p_button_6_ng_template_1_ng_container_1_span_2_1_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵtemplate(0, FileUpload_div_0_ng_container_4_p_button_6_ng_template_1_ng_container_1_span_2_1_ng_template_0_Template, 0, 0, "ng-template");
  }
}
function FileUpload_div_0_ng_container_4_p_button_6_ng_template_1_ng_container_1_span_2_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵelementStart(0, "span", 8);
    ɵɵtemplate(1, FileUpload_div_0_ng_container_4_p_button_6_ng_template_1_ng_container_1_span_2_1_Template, 1, 0, null, 11);
    ɵɵelementEnd();
  }
  if (rf & 2) {
    let tmp_11_0;
    const ctx_r1 = ɵɵnextContext(6);
    ɵɵproperty("pBind", (tmp_11_0 = ctx_r1.ptm("pcUploadButton")) == null ? null : tmp_11_0.icon);
    ɵɵattribute("aria-hidden", true);
    ɵɵadvance();
    ɵɵproperty("ngTemplateOutlet", ctx_r1.uploadIconTemplate || ctx_r1._uploadIconTemplate);
  }
}
function FileUpload_div_0_ng_container_4_p_button_6_ng_template_1_ng_container_1_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵelementContainerStart(0);
    ɵɵtemplate(1, FileUpload_div_0_ng_container_4_p_button_6_ng_template_1_ng_container_1__svg_svg_1_Template, 1, 1, "svg", 21)(2, FileUpload_div_0_ng_container_4_p_button_6_ng_template_1_ng_container_1_span_2_Template, 2, 3, "span", 16);
    ɵɵelementContainerEnd();
  }
  if (rf & 2) {
    const ctx_r1 = ɵɵnextContext(5);
    ɵɵadvance();
    ɵɵproperty("ngIf", !ctx_r1.uploadIconTemplate && !ctx_r1._uploadIconTemplate);
    ɵɵadvance();
    ɵɵproperty("ngIf", ctx_r1.uploadIconTemplate || ctx_r1._uploadIconTemplate);
  }
}
function FileUpload_div_0_ng_container_4_p_button_6_ng_template_1_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵtemplate(0, FileUpload_div_0_ng_container_4_p_button_6_ng_template_1_span_0_Template, 1, 3, "span", 19)(1, FileUpload_div_0_ng_container_4_p_button_6_ng_template_1_ng_container_1_Template, 3, 2, "ng-container", 9);
  }
  if (rf & 2) {
    const ctx_r1 = ɵɵnextContext(4);
    ɵɵproperty("ngIf", ctx_r1.uploadIcon);
    ɵɵadvance();
    ɵɵproperty("ngIf", !ctx_r1.uploadIcon);
  }
}
function FileUpload_div_0_ng_container_4_p_button_6_Template(rf, ctx) {
  if (rf & 1) {
    const _r4 = ɵɵgetCurrentView();
    ɵɵelementStart(0, "p-button", 18);
    ɵɵlistener("onClick", function FileUpload_div_0_ng_container_4_p_button_6_Template_p_button_onClick_0_listener() {
      ɵɵrestoreView(_r4);
      const ctx_r1 = ɵɵnextContext(3);
      return ɵɵresetView(ctx_r1.upload());
    });
    ɵɵtemplate(1, FileUpload_div_0_ng_container_4_p_button_6_ng_template_1_Template, 2, 2, "ng-template", null, 2, ɵɵtemplateRefExtractor);
    ɵɵelementEnd();
  }
  if (rf & 2) {
    const ctx_r1 = ɵɵnextContext(3);
    ɵɵproperty("label", ctx_r1.uploadButtonLabel)("disabled", !ctx_r1.hasFiles() || ctx_r1.isFileLimitExceeded())("styleClass", ctx_r1.cn(ctx_r1.cx("pcUploadButton"), ctx_r1.uploadStyleClass))("buttonProps", ctx_r1.uploadButtonProps)("pt", ctx_r1.ptm("pcUploadButton"))("unstyled", ctx_r1.unstyled());
  }
}
function FileUpload_div_0_ng_container_4_p_button_7_ng_template_1_span_0_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵelement(0, "span", 24);
  }
  if (rf & 2) {
    const ctx_r1 = ɵɵnextContext(5);
    ɵɵproperty("ngClass", ctx_r1.cancelIcon);
  }
}
function FileUpload_div_0_ng_container_4_p_button_7_ng_template_1_ng_container_1__svg_svg_1_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵnamespaceSVG();
    ɵɵelement(0, "svg", 26);
  }
  if (rf & 2) {
    ɵɵattribute("aria-hidden", true);
  }
}
function FileUpload_div_0_ng_container_4_p_button_7_ng_template_1_ng_container_1_span_2_1_ng_template_0_Template(rf, ctx) {
}
function FileUpload_div_0_ng_container_4_p_button_7_ng_template_1_ng_container_1_span_2_1_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵtemplate(0, FileUpload_div_0_ng_container_4_p_button_7_ng_template_1_ng_container_1_span_2_1_ng_template_0_Template, 0, 0, "ng-template");
  }
}
function FileUpload_div_0_ng_container_4_p_button_7_ng_template_1_ng_container_1_span_2_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵelementStart(0, "span");
    ɵɵtemplate(1, FileUpload_div_0_ng_container_4_p_button_7_ng_template_1_ng_container_1_span_2_1_Template, 1, 0, null, 11);
    ɵɵelementEnd();
  }
  if (rf & 2) {
    const ctx_r1 = ɵɵnextContext(6);
    ɵɵattribute("aria-hidden", true);
    ɵɵadvance();
    ɵɵproperty("ngTemplateOutlet", ctx_r1.cancelIconTemplate || ctx_r1._cancelIconTemplate);
  }
}
function FileUpload_div_0_ng_container_4_p_button_7_ng_template_1_ng_container_1_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵelementContainerStart(0);
    ɵɵtemplate(1, FileUpload_div_0_ng_container_4_p_button_7_ng_template_1_ng_container_1__svg_svg_1_Template, 1, 1, "svg", 25)(2, FileUpload_div_0_ng_container_4_p_button_7_ng_template_1_ng_container_1_span_2_Template, 2, 2, "span", 9);
    ɵɵelementContainerEnd();
  }
  if (rf & 2) {
    const ctx_r1 = ɵɵnextContext(5);
    ɵɵadvance();
    ɵɵproperty("ngIf", !ctx_r1.cancelIconTemplate && !ctx_r1._cancelIconTemplate);
    ɵɵadvance();
    ɵɵproperty("ngIf", ctx_r1.cancelIconTemplate || ctx_r1._cancelIconTemplate);
  }
}
function FileUpload_div_0_ng_container_4_p_button_7_ng_template_1_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵtemplate(0, FileUpload_div_0_ng_container_4_p_button_7_ng_template_1_span_0_Template, 1, 1, "span", 23)(1, FileUpload_div_0_ng_container_4_p_button_7_ng_template_1_ng_container_1_Template, 3, 2, "ng-container", 9);
  }
  if (rf & 2) {
    const ctx_r1 = ɵɵnextContext(4);
    ɵɵproperty("ngIf", ctx_r1.cancelIcon);
    ɵɵadvance();
    ɵɵproperty("ngIf", !ctx_r1.cancelIcon);
  }
}
function FileUpload_div_0_ng_container_4_p_button_7_Template(rf, ctx) {
  if (rf & 1) {
    const _r5 = ɵɵgetCurrentView();
    ɵɵelementStart(0, "p-button", 18);
    ɵɵlistener("onClick", function FileUpload_div_0_ng_container_4_p_button_7_Template_p_button_onClick_0_listener() {
      ɵɵrestoreView(_r5);
      const ctx_r1 = ɵɵnextContext(3);
      return ɵɵresetView(ctx_r1.clear());
    });
    ɵɵtemplate(1, FileUpload_div_0_ng_container_4_p_button_7_ng_template_1_Template, 2, 2, "ng-template", null, 2, ɵɵtemplateRefExtractor);
    ɵɵelementEnd();
  }
  if (rf & 2) {
    const ctx_r1 = ɵɵnextContext(3);
    ɵɵproperty("label", ctx_r1.cancelButtonLabel)("disabled", !ctx_r1.hasFiles() || ctx_r1.uploading)("styleClass", ctx_r1.cn(ctx_r1.cx("pcCancelButton"), ctx_r1.cancelStyleClass))("buttonProps", ctx_r1.cancelButtonProps)("pt", ctx_r1.ptm("pcCancelButton"))("unstyled", ctx_r1.unstyled());
  }
}
function FileUpload_div_0_ng_container_4_Template(rf, ctx) {
  if (rf & 1) {
    const _r3 = ɵɵgetCurrentView();
    ɵɵelementContainerStart(0);
    ɵɵelementStart(1, "p-button", 13);
    ɵɵlistener("focus", function FileUpload_div_0_ng_container_4_Template_p_button_focus_1_listener() {
      ɵɵrestoreView(_r3);
      const ctx_r1 = ɵɵnextContext(2);
      return ɵɵresetView(ctx_r1.onFocus());
    })("blur", function FileUpload_div_0_ng_container_4_Template_p_button_blur_1_listener() {
      ɵɵrestoreView(_r3);
      const ctx_r1 = ɵɵnextContext(2);
      return ɵɵresetView(ctx_r1.onBlur());
    })("onClick", function FileUpload_div_0_ng_container_4_Template_p_button_onClick_1_listener() {
      ɵɵrestoreView(_r3);
      const ctx_r1 = ɵɵnextContext(2);
      return ɵɵresetView(ctx_r1.choose());
    })("keydown.enter", function FileUpload_div_0_ng_container_4_Template_p_button_keydown_enter_1_listener() {
      ɵɵrestoreView(_r3);
      const ctx_r1 = ɵɵnextContext(2);
      return ɵɵresetView(ctx_r1.choose());
    });
    ɵɵelementStart(2, "input", 7, 0);
    ɵɵlistener("change", function FileUpload_div_0_ng_container_4_Template_input_change_2_listener($event) {
      ɵɵrestoreView(_r3);
      const ctx_r1 = ɵɵnextContext(2);
      return ɵɵresetView(ctx_r1.onFileSelect($event));
    });
    ɵɵelementEnd();
    ɵɵtemplate(4, FileUpload_div_0_ng_container_4_ng_template_4_Template, 2, 2, "ng-template", null, 2, ɵɵtemplateRefExtractor);
    ɵɵelementEnd();
    ɵɵtemplate(6, FileUpload_div_0_ng_container_4_p_button_6_Template, 3, 6, "p-button", 14)(7, FileUpload_div_0_ng_container_4_p_button_7_Template, 3, 6, "p-button", 14);
    ɵɵelementContainerEnd();
  }
  if (rf & 2) {
    const ctx_r1 = ɵɵnextContext(2);
    ɵɵadvance();
    ɵɵproperty("styleClass", ctx_r1.cn(ctx_r1.cx("pcChooseButton"), ctx_r1.chooseStyleClass))("disabled", ctx_r1.disabled || ctx_r1.isChooseDisabled())("label", ctx_r1.chooseButtonLabel)("buttonProps", ctx_r1.chooseButtonProps)("pt", ctx_r1.ptm("pcChooseButton"))("unstyled", ctx_r1.unstyled());
    ɵɵadvance();
    ɵɵproperty("multiple", ctx_r1.multiple)("accept", ctx_r1.accept)("disabled", ctx_r1.disabled || ctx_r1.isChooseDisabled())("pBind", ctx_r1.ptm("input"));
    ɵɵattribute("aria-label", ctx_r1.browseFilesLabel)("title", "");
    ɵɵadvance(4);
    ɵɵproperty("ngIf", !ctx_r1.auto && ctx_r1.showUploadButton);
    ɵɵadvance();
    ɵɵproperty("ngIf", !ctx_r1.auto && ctx_r1.showCancelButton);
  }
}
function FileUpload_div_0_ng_container_5_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵelementContainer(0);
  }
}
function FileUpload_div_0_ng_container_6_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵelementContainer(0);
  }
}
function FileUpload_div_0_Conditional_9_ng_container_0_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵelementContainer(0);
  }
}
function FileUpload_div_0_Conditional_9_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵtemplate(0, FileUpload_div_0_Conditional_9_ng_container_0_Template, 1, 0, "ng-container", 10);
  }
  if (rf & 2) {
    const ctx_r1 = ɵɵnextContext(2);
    ɵɵproperty("ngTemplateOutlet", ctx_r1.contentTemplate || ctx_r1._contentTemplate)("ngTemplateOutletContext", ɵɵpureFunction8(2, _c14, ctx_r1.files, ctx_r1.uploadedFiles, ctx_r1.choose.bind(ctx_r1), ctx_r1.clear.bind(ctx_r1), ctx_r1.removeUploadedFile.bind(ctx_r1), ctx_r1.remove.bind(ctx_r1), ctx_r1.progress, ctx_r1.msgs));
  }
}
function FileUpload_div_0_Conditional_10_p_progressbar_0_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵelement(0, "p-progressbar", 30);
  }
  if (rf & 2) {
    const ctx_r1 = ɵɵnextContext(3);
    ɵɵproperty("value", ctx_r1.progress)("showValue", false)("pt", ctx_r1.ptm("pcProgressBar"));
  }
}
function FileUpload_div_0_Conditional_10_For_2_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵelement(0, "p-message", 28);
  }
  if (rf & 2) {
    const message_r6 = ctx.$implicit;
    const ctx_r1 = ɵɵnextContext(3);
    ɵɵproperty("severity", message_r6.severity)("text", message_r6.text)("pt", ctx_r1.ptm("pcMessage"))("unstyled", ctx_r1.unstyled());
  }
}
function FileUpload_div_0_Conditional_10_Conditional_3_ng_template_1_Template(rf, ctx) {
}
function FileUpload_div_0_Conditional_10_Conditional_3_Conditional_2_Template(rf, ctx) {
  if (rf & 1) {
    const _r7 = ɵɵgetCurrentView();
    ɵɵelementStart(0, "div", 33);
    ɵɵlistener("onRemove", function FileUpload_div_0_Conditional_10_Conditional_3_Conditional_2_Template_div_onRemove_0_listener($event) {
      ɵɵrestoreView(_r7);
      const ctx_r1 = ɵɵnextContext(4);
      return ɵɵresetView(ctx_r1.onRemoveClick($event));
    });
    ɵɵelementEnd();
  }
  if (rf & 2) {
    const ctx_r1 = ɵɵnextContext(4);
    ɵɵproperty("unstyled", ctx_r1.unstyled())("files", ctx_r1.files)("badgeValue", ctx_r1.pendingLabel)("previewWidth", ctx_r1.previewWidth)("fileRemoveIconTemplate", ctx_r1.cancelIconTemplate || ctx_r1._cancelIconTemplate);
  }
}
function FileUpload_div_0_Conditional_10_Conditional_3_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵelementStart(0, "div", 8);
    ɵɵtemplate(1, FileUpload_div_0_Conditional_10_Conditional_3_ng_template_1_Template, 0, 0, "ng-template", 31);
    ɵɵconditionalCreate(2, FileUpload_div_0_Conditional_10_Conditional_3_Conditional_2_Template, 1, 5, "div", 32);
    ɵɵelementEnd();
  }
  if (rf & 2) {
    const ctx_r1 = ɵɵnextContext(3);
    ɵɵclassMap(ctx_r1.cx("fileList"));
    ɵɵproperty("pBind", ctx_r1.ptm("fileList"));
    ɵɵadvance();
    ɵɵproperty("ngForOf", ctx_r1.files)("ngForTemplate", ctx_r1.fileTemplate || ctx_r1._fileTemplate);
    ɵɵadvance();
    ɵɵconditional(!ctx_r1.fileTemplate && !ctx_r1._fileTemplate ? 2 : -1);
  }
}
function FileUpload_div_0_Conditional_10_Conditional_4_ng_template_1_Template(rf, ctx) {
}
function FileUpload_div_0_Conditional_10_Conditional_4_Conditional_2_Template(rf, ctx) {
  if (rf & 1) {
    const _r8 = ɵɵgetCurrentView();
    ɵɵelementStart(0, "div", 35);
    ɵɵlistener("onRemove", function FileUpload_div_0_Conditional_10_Conditional_4_Conditional_2_Template_div_onRemove_0_listener($event) {
      ɵɵrestoreView(_r8);
      const ctx_r1 = ɵɵnextContext(4);
      return ɵɵresetView(ctx_r1.onRemoveUploadedFileClick($event));
    });
    ɵɵelementEnd();
  }
  if (rf & 2) {
    const ctx_r1 = ɵɵnextContext(4);
    ɵɵproperty("unstyled", ctx_r1.unstyled())("files", ctx_r1.uploadedFiles)("badgeValue", ctx_r1.completedLabel())("previewWidth", ctx_r1.previewWidth)("fileRemoveIconTemplate", ctx_r1.cancelIconTemplate || ctx_r1._cancelIconTemplate);
  }
}
function FileUpload_div_0_Conditional_10_Conditional_4_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵelementStart(0, "div", 8);
    ɵɵtemplate(1, FileUpload_div_0_Conditional_10_Conditional_4_ng_template_1_Template, 0, 0, "ng-template", 31);
    ɵɵconditionalCreate(2, FileUpload_div_0_Conditional_10_Conditional_4_Conditional_2_Template, 1, 5, "div", 34);
    ɵɵelementEnd();
  }
  if (rf & 2) {
    const ctx_r1 = ɵɵnextContext(3);
    ɵɵclassMap(ctx_r1.cx("fileList"));
    ɵɵproperty("pBind", ctx_r1.ptm("fileList"));
    ɵɵadvance();
    ɵɵproperty("ngForOf", ctx_r1.uploadedFiles)("ngForTemplate", ctx_r1.fileTemplate || ctx_r1._fileTemplate);
    ɵɵadvance();
    ɵɵconditional(!ctx_r1.fileTemplate && !ctx_r1._fileTemplate ? 2 : -1);
  }
}
function FileUpload_div_0_Conditional_10_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵtemplate(0, FileUpload_div_0_Conditional_10_p_progressbar_0_Template, 1, 3, "p-progressbar", 27);
    ɵɵrepeaterCreate(1, FileUpload_div_0_Conditional_10_For_2_Template, 1, 4, "p-message", 28, ɵɵrepeaterTrackByIdentity);
    ɵɵconditionalCreate(3, FileUpload_div_0_Conditional_10_Conditional_3_Template, 3, 6, "div", 29);
    ɵɵconditionalCreate(4, FileUpload_div_0_Conditional_10_Conditional_4_Template, 3, 6, "div", 29);
  }
  if (rf & 2) {
    const ctx_r1 = ɵɵnextContext(2);
    ɵɵproperty("ngIf", ctx_r1.hasFiles());
    ɵɵadvance();
    ɵɵrepeater(ctx_r1.msgs);
    ɵɵadvance(2);
    ɵɵconditional(ctx_r1.hasFiles() ? 3 : -1);
    ɵɵadvance();
    ɵɵconditional(ctx_r1.hasUploadedFiles() ? 4 : -1);
  }
}
function FileUpload_div_0_Conditional_11_ng_container_0_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵelementContainer(0, 8);
  }
  if (rf & 2) {
    const ctx_r1 = ɵɵnextContext(3);
    ɵɵproperty("pBind", ctx_r1.ptm("empty"));
  }
}
function FileUpload_div_0_Conditional_11_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵtemplate(0, FileUpload_div_0_Conditional_11_ng_container_0_Template, 1, 1, "ng-container", 36);
  }
  if (rf & 2) {
    const ctx_r1 = ɵɵnextContext(2);
    ɵɵproperty("ngTemplateOutlet", ctx_r1.emptyTemplate || ctx_r1._emptyTemplate);
  }
}
function FileUpload_div_0_Template(rf, ctx) {
  if (rf & 1) {
    const _r1 = ɵɵgetCurrentView();
    ɵɵelementStart(0, "div", 6)(1, "input", 7, 0);
    ɵɵlistener("change", function FileUpload_div_0_Template_input_change_1_listener($event) {
      ɵɵrestoreView(_r1);
      const ctx_r1 = ɵɵnextContext();
      return ɵɵresetView(ctx_r1.onFileSelect($event));
    });
    ɵɵelementEnd();
    ɵɵelementStart(3, "div", 8);
    ɵɵtemplate(4, FileUpload_div_0_ng_container_4_Template, 8, 14, "ng-container", 9)(5, FileUpload_div_0_ng_container_5_Template, 1, 0, "ng-container", 10)(6, FileUpload_div_0_ng_container_6_Template, 1, 0, "ng-container", 11);
    ɵɵelementEnd();
    ɵɵelementStart(7, "div", 12, 1);
    ɵɵlistener("dragenter", function FileUpload_div_0_Template_div_dragenter_7_listener($event) {
      ɵɵrestoreView(_r1);
      const ctx_r1 = ɵɵnextContext();
      return ɵɵresetView(ctx_r1.onDragEnter($event));
    })("dragleave", function FileUpload_div_0_Template_div_dragleave_7_listener($event) {
      ɵɵrestoreView(_r1);
      const ctx_r1 = ɵɵnextContext();
      return ɵɵresetView(ctx_r1.onDragLeave($event));
    })("drop", function FileUpload_div_0_Template_div_drop_7_listener($event) {
      ɵɵrestoreView(_r1);
      const ctx_r1 = ɵɵnextContext();
      return ɵɵresetView(ctx_r1.onDrop($event));
    });
    ɵɵconditionalCreate(9, FileUpload_div_0_Conditional_9_Template, 1, 11, "ng-container")(10, FileUpload_div_0_Conditional_10_Template, 5, 3);
    ɵɵconditionalCreate(11, FileUpload_div_0_Conditional_11_Template, 1, 1, "ng-container", 8);
    ɵɵelementEnd()();
  }
  if (rf & 2) {
    const ctx_r1 = ɵɵnextContext();
    ɵɵclassMap(ctx_r1.cn(ctx_r1.cx("root"), ctx_r1.styleClass));
    ɵɵproperty("ngStyle", ctx_r1.style)("pBind", ctx_r1.ptm("root"));
    ɵɵadvance();
    ɵɵproperty("multiple", ctx_r1.multiple)("accept", ctx_r1.accept)("disabled", ctx_r1.disabled || ctx_r1.isChooseDisabled())("pBind", ctx_r1.ptm("input"));
    ɵɵattribute("aria-label", ctx_r1.browseFilesLabel)("title", "");
    ɵɵadvance(2);
    ɵɵclassMap(ctx_r1.cx("header"));
    ɵɵproperty("pBind", ctx_r1.ptm("header"));
    ɵɵadvance();
    ɵɵproperty("ngIf", !ctx_r1.headerTemplate && !ctx_r1._headerTemplate);
    ɵɵadvance();
    ɵɵproperty("ngTemplateOutlet", ctx_r1.headerTemplate || ctx_r1._headerTemplate)("ngTemplateOutletContext", ɵɵpureFunction5(22, _c132, ctx_r1.files, ctx_r1.uploadedFiles, ctx_r1.choose.bind(ctx_r1), ctx_r1.clear.bind(ctx_r1), ctx_r1.upload.bind(ctx_r1)));
    ɵɵadvance();
    ɵɵproperty("ngTemplateOutlet", ctx_r1.toolbarTemplate || ctx_r1._toolbarTemplate);
    ɵɵadvance();
    ɵɵclassMap(ctx_r1.cx("content"));
    ɵɵproperty("pBind", ctx_r1.ptm("content"));
    ɵɵadvance(2);
    ɵɵconditional(ctx_r1.contentTemplate || ctx_r1._contentTemplate ? 9 : 10);
    ɵɵadvance(2);
    ɵɵconditional((ctx_r1.emptyTemplate || ctx_r1._emptyTemplate) && !ctx_r1.hasFiles() && !ctx_r1.hasUploadedFiles() ? 11 : -1);
  }
}
function FileUpload_div_1_For_2_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵelement(0, "p-message", 28);
  }
  if (rf & 2) {
    const message_r10 = ctx.$implicit;
    const ctx_r1 = ɵɵnextContext(2);
    ɵɵproperty("severity", message_r10.severity)("text", message_r10.text)("pt", ctx_r1.ptm("pcMessage"))("unstyled", ctx_r1.unstyled());
  }
}
function FileUpload_div_1_ng_template_5_Conditional_0_span_0_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵelement(0, "span", 40);
  }
  if (rf & 2) {
    let tmp_7_0;
    const ctx_r1 = ɵɵnextContext(4);
    ɵɵproperty("ngClass", ctx_r1.uploadIcon)("pBind", (tmp_7_0 = ctx_r1.ptm("pcChooseButton")) == null ? null : tmp_7_0.icon);
  }
}
function FileUpload_div_1_ng_template_5_Conditional_0_ng_container_1__svg_svg_1_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵnamespaceSVG();
    ɵɵelement(0, "svg", 22);
  }
  if (rf & 2) {
    let tmp_8_0;
    const ctx_r1 = ɵɵnextContext(5);
    ɵɵclassMap("p-button-icon p-button-icon-left");
    ɵɵproperty("pBind", (tmp_8_0 = ctx_r1.ptm("pcChooseButton")) == null ? null : tmp_8_0.icon);
  }
}
function FileUpload_div_1_ng_template_5_Conditional_0_ng_container_1_span_2_1_ng_template_0_Template(rf, ctx) {
}
function FileUpload_div_1_ng_template_5_Conditional_0_ng_container_1_span_2_1_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵtemplate(0, FileUpload_div_1_ng_template_5_Conditional_0_ng_container_1_span_2_1_ng_template_0_Template, 0, 0, "ng-template");
  }
}
function FileUpload_div_1_ng_template_5_Conditional_0_ng_container_1_span_2_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵelementStart(0, "span", 43);
    ɵɵtemplate(1, FileUpload_div_1_ng_template_5_Conditional_0_ng_container_1_span_2_1_Template, 1, 0, null, 11);
    ɵɵelementEnd();
  }
  if (rf & 2) {
    let tmp_7_0;
    const ctx_r1 = ɵɵnextContext(5);
    ɵɵproperty("pBind", (tmp_7_0 = ctx_r1.ptm("pcChooseButton")) == null ? null : tmp_7_0.icon);
    ɵɵadvance();
    ɵɵproperty("ngTemplateOutlet", ctx_r1._uploadIconTemplate || ctx_r1.uploadIconTemplate);
  }
}
function FileUpload_div_1_ng_template_5_Conditional_0_ng_container_1_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵelementContainerStart(0);
    ɵɵtemplate(1, FileUpload_div_1_ng_template_5_Conditional_0_ng_container_1__svg_svg_1_Template, 1, 3, "svg", 41)(2, FileUpload_div_1_ng_template_5_Conditional_0_ng_container_1_span_2_Template, 2, 2, "span", 42);
    ɵɵelementContainerEnd();
  }
  if (rf & 2) {
    const ctx_r1 = ɵɵnextContext(4);
    ɵɵadvance();
    ɵɵproperty("ngIf", !ctx_r1.uploadIconTemplate && !ctx_r1._uploadIconTemplate);
    ɵɵadvance();
    ɵɵproperty("ngIf", ctx_r1._uploadIconTemplate || ctx_r1.uploadIconTemplate);
  }
}
function FileUpload_div_1_ng_template_5_Conditional_0_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵtemplate(0, FileUpload_div_1_ng_template_5_Conditional_0_span_0_Template, 1, 2, "span", 39)(1, FileUpload_div_1_ng_template_5_Conditional_0_ng_container_1_Template, 3, 2, "ng-container", 9);
  }
  if (rf & 2) {
    const ctx_r1 = ɵɵnextContext(3);
    ɵɵproperty("ngIf", ctx_r1.uploadIcon);
    ɵɵadvance();
    ɵɵproperty("ngIf", !ctx_r1.uploadIcon);
  }
}
function FileUpload_div_1_ng_template_5_Conditional_1_span_0_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵelement(0, "span", 45);
  }
  if (rf & 2) {
    let tmp_7_0;
    const ctx_r1 = ɵɵnextContext(4);
    ɵɵproperty("ngClass", ctx_r1.chooseIcon)("pBind", (tmp_7_0 = ctx_r1.ptm("pcChooseButton")) == null ? null : tmp_7_0.icon);
  }
}
function FileUpload_div_1_ng_template_5_Conditional_1_ng_container_1__svg_svg_1_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵnamespaceSVG();
    ɵɵelement(0, "svg", 17);
  }
  if (rf & 2) {
    let tmp_7_0;
    const ctx_r1 = ɵɵnextContext(5);
    ɵɵproperty("pBind", (tmp_7_0 = ctx_r1.ptm("pcChooseButton")) == null ? null : tmp_7_0.icon);
  }
}
function FileUpload_div_1_ng_template_5_Conditional_1_ng_container_1_2_ng_template_0_Template(rf, ctx) {
}
function FileUpload_div_1_ng_template_5_Conditional_1_ng_container_1_2_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵtemplate(0, FileUpload_div_1_ng_template_5_Conditional_1_ng_container_1_2_ng_template_0_Template, 0, 0, "ng-template");
  }
}
function FileUpload_div_1_ng_template_5_Conditional_1_ng_container_1_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵelementContainerStart(0);
    ɵɵtemplate(1, FileUpload_div_1_ng_template_5_Conditional_1_ng_container_1__svg_svg_1_Template, 1, 1, "svg", 15)(2, FileUpload_div_1_ng_template_5_Conditional_1_ng_container_1_2_Template, 1, 0, null, 11);
    ɵɵelementContainerEnd();
  }
  if (rf & 2) {
    const ctx_r1 = ɵɵnextContext(4);
    ɵɵadvance();
    ɵɵproperty("ngIf", !ctx_r1.chooseIconTemplate && !ctx_r1._chooseIconTemplate);
    ɵɵadvance();
    ɵɵproperty("ngTemplateOutlet", ctx_r1.chooseIconTemplate || ctx_r1._chooseIconTemplate);
  }
}
function FileUpload_div_1_ng_template_5_Conditional_1_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵtemplate(0, FileUpload_div_1_ng_template_5_Conditional_1_span_0_Template, 1, 2, "span", 44)(1, FileUpload_div_1_ng_template_5_Conditional_1_ng_container_1_Template, 3, 2, "ng-container", 9);
  }
  if (rf & 2) {
    const ctx_r1 = ɵɵnextContext(3);
    ɵɵproperty("ngIf", ctx_r1.chooseIcon);
    ɵɵadvance();
    ɵɵproperty("ngIf", !ctx_r1.chooseIcon);
  }
}
function FileUpload_div_1_ng_template_5_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵconditionalCreate(0, FileUpload_div_1_ng_template_5_Conditional_0_Template, 2, 2)(1, FileUpload_div_1_ng_template_5_Conditional_1_Template, 2, 2);
  }
  if (rf & 2) {
    const ctx_r1 = ɵɵnextContext(2);
    ɵɵconditional(ctx_r1.hasFiles() && !ctx_r1.auto ? 0 : 1);
  }
}
function FileUpload_div_1_Conditional_9_Conditional_0_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵelementStart(0, "span");
    ɵɵtext(1);
    ɵɵelementEnd();
  }
  if (rf & 2) {
    const ctx_r1 = ɵɵnextContext(3);
    ɵɵadvance();
    ɵɵtextInterpolate1(" ", ctx_r1.basicFileChosenLabel(), " ");
  }
}
function FileUpload_div_1_Conditional_9_Conditional_1_ng_container_0_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵelementContainer(0);
  }
}
function FileUpload_div_1_Conditional_9_Conditional_1_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵtemplate(0, FileUpload_div_1_Conditional_9_Conditional_1_ng_container_0_Template, 1, 0, "ng-container", 10);
  }
  if (rf & 2) {
    const ctx_r1 = ɵɵnextContext(3);
    ɵɵproperty("ngTemplateOutlet", ctx_r1.fileLabelTemplate || ctx_r1._fileLabelTemplate)("ngTemplateOutletContext", ɵɵpureFunction1(2, _c15, ctx_r1.files));
  }
}
function FileUpload_div_1_Conditional_9_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵconditionalCreate(0, FileUpload_div_1_Conditional_9_Conditional_0_Template, 2, 1, "span")(1, FileUpload_div_1_Conditional_9_Conditional_1_Template, 1, 4, "ng-container");
  }
  if (rf & 2) {
    const ctx_r1 = ɵɵnextContext(2);
    ɵɵconditional(!ctx_r1.fileLabelTemplate && !ctx_r1._fileLabelTemplate ? 0 : 1);
  }
}
function FileUpload_div_1_Template(rf, ctx) {
  if (rf & 1) {
    const _r9 = ɵɵgetCurrentView();
    ɵɵelementStart(0, "div", 8);
    ɵɵrepeaterCreate(1, FileUpload_div_1_For_2_Template, 1, 4, "p-message", 28, ɵɵrepeaterTrackByIdentity);
    ɵɵelementStart(3, "div", 8)(4, "p-button", 37);
    ɵɵlistener("onClick", function FileUpload_div_1_Template_p_button_onClick_4_listener() {
      ɵɵrestoreView(_r9);
      const ctx_r1 = ɵɵnextContext();
      return ɵɵresetView(ctx_r1.onBasicUploaderClick());
    })("keydown", function FileUpload_div_1_Template_p_button_keydown_4_listener($event) {
      ɵɵrestoreView(_r9);
      const ctx_r1 = ɵɵnextContext();
      return ɵɵresetView(ctx_r1.onBasicKeydown($event));
    });
    ɵɵtemplate(5, FileUpload_div_1_ng_template_5_Template, 2, 1, "ng-template", null, 2, ɵɵtemplateRefExtractor);
    ɵɵelementStart(7, "input", 38, 3);
    ɵɵlistener("change", function FileUpload_div_1_Template_input_change_7_listener($event) {
      ɵɵrestoreView(_r9);
      const ctx_r1 = ɵɵnextContext();
      return ɵɵresetView(ctx_r1.onFileSelect($event));
    })("focus", function FileUpload_div_1_Template_input_focus_7_listener() {
      ɵɵrestoreView(_r9);
      const ctx_r1 = ɵɵnextContext();
      return ɵɵresetView(ctx_r1.onFocus());
    })("blur", function FileUpload_div_1_Template_input_blur_7_listener() {
      ɵɵrestoreView(_r9);
      const ctx_r1 = ɵɵnextContext();
      return ɵɵresetView(ctx_r1.onBlur());
    });
    ɵɵelementEnd()();
    ɵɵconditionalCreate(9, FileUpload_div_1_Conditional_9_Template, 2, 1);
    ɵɵelementEnd()();
  }
  if (rf & 2) {
    const ctx_r1 = ɵɵnextContext();
    ɵɵclassMap(ctx_r1.cn(ctx_r1.cx("root"), ctx_r1.styleClass));
    ɵɵproperty("pBind", ctx_r1.ptm("root"));
    ɵɵadvance();
    ɵɵrepeater(ctx_r1.msgs);
    ɵɵadvance(2);
    ɵɵclassMap(ctx_r1.cx("basicContent"));
    ɵɵproperty("pBind", ctx_r1.ptm("basicContent"));
    ɵɵadvance();
    ɵɵstyleMap(ctx_r1.style);
    ɵɵproperty("styleClass", ctx_r1.cn(ctx_r1.cx("pcChooseButton"), ctx_r1.chooseStyleClass))("disabled", ctx_r1.disabled)("label", ctx_r1.chooseButtonLabel)("buttonProps", ctx_r1.chooseButtonProps)("pt", ctx_r1.ptm("pcChooseButton"))("unstyled", ctx_r1.unstyled());
    ɵɵadvance(3);
    ɵɵproperty("accept", ctx_r1.accept)("multiple", ctx_r1.multiple)("disabled", ctx_r1.disabled)("pBind", ctx_r1.ptm("input"));
    ɵɵattribute("aria-label", ctx_r1.browseFilesLabel);
    ɵɵadvance(2);
    ɵɵconditional(!ctx_r1.auto ? 9 : -1);
  }
}
var classes3 = {
  root: ({
    instance
  }) => `p-fileupload p-fileupload-${instance.mode} p-component`,
  header: "p-fileupload-header",
  pcChooseButton: "p-fileupload-choose-button",
  pcUploadButton: "p-fileupload-upload-button",
  pcCancelButton: "p-fileupload-cancel-button",
  content: "p-fileupload-content",
  fileList: "p-fileupload-file-list",
  file: "p-fileupload-file",
  fileThumbnail: "p-fileupload-file-thumbnail",
  fileInfo: "p-fileupload-file-info",
  fileName: "p-fileupload-file-name",
  fileSize: "p-fileupload-file-size",
  pcFileBadge: "p-fileupload-file-badge",
  fileActions: "p-fileupload-file-actions",
  pcFileRemoveButton: "p-fileupload-file-remove-button",
  basicContent: "p-fileupload-basic-content"
};
var FileUploadStyle = class _FileUploadStyle extends BaseStyle {
  name = "fileupload";
  style = style3;
  classes = classes3;
  static ɵfac = /* @__PURE__ */ (() => {
    let ɵFileUploadStyle_BaseFactory;
    return function FileUploadStyle_Factory(__ngFactoryType__) {
      return (ɵFileUploadStyle_BaseFactory || (ɵFileUploadStyle_BaseFactory = ɵɵgetInheritedFactory(_FileUploadStyle)))(__ngFactoryType__ || _FileUploadStyle);
    };
  })();
  static ɵprov = ɵɵdefineInjectable({
    token: _FileUploadStyle,
    factory: _FileUploadStyle.ɵfac
  });
};
(() => {
  (typeof ngDevMode === "undefined" || ngDevMode) && setClassMetadata(FileUploadStyle, [{
    type: Injectable
  }], null, null);
})();
var FileUploadClasses;
(function(FileUploadClasses2) {
  FileUploadClasses2["root"] = "p-fileupload";
  FileUploadClasses2["header"] = "p-fileupload-header";
  FileUploadClasses2["pcChooseButton"] = "p-fileupload-choose-button";
  FileUploadClasses2["pcUploadButton"] = "p-fileupload-upload-button";
  FileUploadClasses2["pcCancelButton"] = "p-fileupload-cancel-button";
  FileUploadClasses2["content"] = "p-fileupload-content";
  FileUploadClasses2["fileList"] = "p-fileupload-file-list";
  FileUploadClasses2["file"] = "p-fileupload-file";
  FileUploadClasses2["fileThumbnail"] = "p-fileupload-file-thumbnail";
  FileUploadClasses2["fileInfo"] = "p-fileupload-file-info";
  FileUploadClasses2["fileName"] = "p-fileupload-file-name";
  FileUploadClasses2["fileSize"] = "p-fileupload-file-size";
  FileUploadClasses2["pcFileBadge"] = "p-fileupload-file-badge";
  FileUploadClasses2["fileActions"] = "p-fileupload-file-actions";
  FileUploadClasses2["pcFileRemoveButton"] = "p-fileupload-file-remove-button";
  FileUploadClasses2["basicContent"] = "p-fileupload-basic-content";
})(FileUploadClasses || (FileUploadClasses = {}));
var FILEUPLOAD_INSTANCE = new InjectionToken("FILEUPLOAD_INSTANCE");
var FileContent = class _FileContent extends BaseComponent {
  _componentStyle = inject(FileUploadStyle);
  $pcFileUpload = inject(FILEUPLOAD_INSTANCE);
  onRemove = output();
  files = input(...ngDevMode ? [void 0, {
    debugName: "files"
  }] : (
    /* istanbul ignore next */
    []
  ));
  badgeSeverity = input("warn", ...ngDevMode ? [{
    debugName: "badgeSeverity"
  }] : (
    /* istanbul ignore next */
    []
  ));
  badgeValue = input(...ngDevMode ? [void 0, {
    debugName: "badgeValue"
  }] : (
    /* istanbul ignore next */
    []
  ));
  previewWidth = input(50, ...ngDevMode ? [{
    debugName: "previewWidth"
  }] : (
    /* istanbul ignore next */
    []
  ));
  fileRemoveIconTemplate = input(...ngDevMode ? [void 0, {
    debugName: "fileRemoveIconTemplate"
  }] : (
    /* istanbul ignore next */
    []
  ));
  onRemoveClick(event, index) {
    this.onRemove.emit({
      event,
      index
    });
  }
  formatSize(bytes) {
    const k = 1024;
    const dm = 3;
    const sizes = this.config.getTranslation(TranslationKeys.FILE_SIZE_TYPES);
    if (bytes === 0) {
      return `0 ${sizes[0]}`;
    }
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    const formattedSize = (bytes / Math.pow(k, i)).toFixed(dm);
    return `${formattedSize} ${sizes[i]}`;
  }
  static ɵfac = /* @__PURE__ */ (() => {
    let ɵFileContent_BaseFactory;
    return function FileContent_Factory(__ngFactoryType__) {
      return (ɵFileContent_BaseFactory || (ɵFileContent_BaseFactory = ɵɵgetInheritedFactory(_FileContent)))(__ngFactoryType__ || _FileContent);
    };
  })();
  static ɵcmp = ɵɵdefineComponent({
    type: _FileContent,
    selectors: [["", "pFileContent", ""]],
    inputs: {
      files: [1, "files"],
      badgeSeverity: [1, "badgeSeverity"],
      badgeValue: [1, "badgeValue"],
      previewWidth: [1, "previewWidth"],
      fileRemoveIconTemplate: [1, "fileRemoveIconTemplate"]
    },
    outputs: {
      onRemove: "onRemove"
    },
    features: [ɵɵProvidersFeature([FileUploadStyle]), ɵɵInheritDefinitionFeature],
    attrs: _c03,
    decls: 2,
    vars: 0,
    consts: [["icon", ""], [3, "class", "pBind"], [3, "pBind"], ["role", "presentation", 3, "src", "width", "pBind"], [3, "value", "severity", "pt", "unstyled"], ["text", "", "rounded", "", "severity", "danger", 3, "onClick", "styleClass", "pt", "unstyled"], ["data-p-icon", "times", 3, "class"], [4, "ngTemplateOutlet", "ngTemplateOutletContext"], ["data-p-icon", "times"]],
    template: function FileContent_Template(rf, ctx) {
      if (rf & 1) {
        ɵɵrepeaterCreate(0, FileContent_For_1_Template, 12, 32, "div", 1, _forTrack0);
      }
      if (rf & 2) {
        ɵɵrepeater(ctx.files());
      }
    },
    dependencies: [CommonModule, NgTemplateOutlet, Badge, Button, TimesIcon, Bind],
    encapsulation: 2,
    changeDetection: 0
  });
};
(() => {
  (typeof ngDevMode === "undefined" || ngDevMode) && setClassMetadata(FileContent, [{
    type: Component,
    args: [{
      selector: "[pFileContent]",
      standalone: true,
      template: `@for (file of files(); track file?.name + '-' + $index; let index = $index) {
        <div [class]="cx('file')" [pBind]="$pcFileUpload.ptm('file')">
            <img role="presentation" [class]="cx('fileThumbnail')" [attr.alt]="file.name" [src]="file.objectURL" [width]="previewWidth()" [pBind]="$pcFileUpload.ptm('fileThumbnail')" />
            <div [class]="cx('fileInfo')" [pBind]="$pcFileUpload.ptm('fileInfo')">
                <div [class]="cx('fileName')" [pBind]="$pcFileUpload.ptm('fileName')">{{ file.name }}</div>
                <span [class]="cx('fileSize')" [pBind]="$pcFileUpload.ptm('fileSize')">{{ formatSize(file.size) }}</span>
            </div>
            <p-badge [value]="badgeValue()" [severity]="badgeSeverity()" [class]="cx('pcFileBadge')" [pt]="$pcFileUpload.ptm('pcFileBadge')" [unstyled]="unstyled()" />
            <div [class]="cx('fileActions')" [pBind]="$pcFileUpload.ptm('fileActions')">
                <p-button (onClick)="onRemoveClick($event, index)" [styleClass]="cx('pcFileRemoveButton')" text rounded severity="danger" [pt]="$pcFileUpload.ptm('pcFileRemoveButton')" [unstyled]="unstyled()">
                    <ng-template #icon let-iconClass="class">
                        @if (fileRemoveIconTemplate()) {
                            <ng-template *ngTemplateOutlet="fileRemoveIconTemplate(); context: { class: iconClass, file: file, index: index }"></ng-template>
                        } @else {
                            <svg data-p-icon="times" [class]="iconClass" [attr.aria-hidden]="true" />
                        }
                    </ng-template>
                </p-button>
            </div>
        </div>
    }`,
      changeDetection: ChangeDetectionStrategy.OnPush,
      providers: [FileUploadStyle],
      imports: [CommonModule, Badge, Button, TimesIcon, Bind]
    }]
  }], null, {
    onRemove: [{
      type: Output,
      args: ["onRemove"]
    }],
    files: [{
      type: Input,
      args: [{
        isSignal: true,
        alias: "files",
        required: false
      }]
    }],
    badgeSeverity: [{
      type: Input,
      args: [{
        isSignal: true,
        alias: "badgeSeverity",
        required: false
      }]
    }],
    badgeValue: [{
      type: Input,
      args: [{
        isSignal: true,
        alias: "badgeValue",
        required: false
      }]
    }],
    previewWidth: [{
      type: Input,
      args: [{
        isSignal: true,
        alias: "previewWidth",
        required: false
      }]
    }],
    fileRemoveIconTemplate: [{
      type: Input,
      args: [{
        isSignal: true,
        alias: "fileRemoveIconTemplate",
        required: false
      }]
    }]
  });
})();
var FileUpload = class _FileUpload extends BaseComponent {
  componentName = "FileUpload";
  bindDirectiveInstance = inject(Bind, {
    self: true
  });
  onAfterViewChecked() {
    this.bindDirectiveInstance.setAttrs(this.ptm("host"));
  }
  /**
   * Name of the request parameter to identify the files at backend.
   * @group Props
   */
  name;
  /**
   * Remote url to upload the files.
   * @group Props
   */
  url;
  /**
   * HTTP method to send the files to the url such as "post" and "put".
   * @group Props
   */
  method = "post";
  /**
   * Used to select multiple files at once from file dialog.
   * @group Props
   */
  multiple;
  /**
   * Comma-separated list of pattern to restrict the allowed file types. Can be any combination of either the MIME types (such as "image/*") or the file extensions (such as ".jpg").
   * @group Props
   */
  accept;
  /**
   * Disables the upload functionality.
   * @group Props
   */
  disabled;
  /**
   * When enabled, upload begins automatically after selection is completed.
   * @group Props
   */
  auto;
  /**
   * Cross-site Access-Control requests should be made using credentials such as cookies, authorization headers or TLS client certificates.
   * @group Props
   */
  withCredentials;
  /**
   * Maximum file size allowed in bytes.
   * @group Props
   */
  maxFileSize;
  /**
   * Summary message of the invalid file size.
   * @group Props
   */
  invalidFileSizeMessageSummary = "{0}: Invalid file size, ";
  /**
   * Detail message of the invalid file size.
   * @group Props
   */
  invalidFileSizeMessageDetail = "maximum upload size is {0}.";
  /**
   * Summary message of the invalid file type.
   * @group Props
   */
  invalidFileTypeMessageSummary = "{0}: Invalid file type, ";
  /**
   * Detail message of the invalid file type.
   * @group Props
   */
  invalidFileTypeMessageDetail = "allowed file types: {0}.";
  /**
   * Detail message of the invalid file type.
   * @group Props
   */
  invalidFileLimitMessageDetail = "limit is {0} at most.";
  /**
   * Summary message of the invalid file type.
   * @group Props
   */
  invalidFileLimitMessageSummary = "Maximum number of files exceeded, ";
  /**
   * Inline style of the element.
   * @group Props
   */
  style;
  /**
   * Class of the element.
   * @group Props
   */
  styleClass;
  /**
   * Width of the image thumbnail in pixels.
   * @group Props
   */
  previewWidth = 50;
  /**
   * Label of the choose button. Defaults to PrimeNG Locale configuration.
   * @group Props
   */
  chooseLabel;
  /**
   * Label of the upload button. Defaults to PrimeNG Locale configuration.
   * @group Props
   */
  uploadLabel;
  /**
   * Label of the cancel button. Defaults to PrimeNG Locale configuration.
   * @group Props
   */
  cancelLabel;
  /**
   * Icon of the choose button.
   * @group Props
   */
  chooseIcon;
  /**
   * Icon of the upload button.
   * @group Props
   */
  uploadIcon;
  /**
   * Icon of the cancel button.
   * @group Props
   */
  cancelIcon;
  /**
   * Whether to show the upload button.
   * @group Props
   */
  showUploadButton = true;
  /**
   * Whether to show the cancel button.
   * @group Props
   */
  showCancelButton = true;
  /**
   * Defines the UI of the component.
   * @group Props
   */
  mode = "advanced";
  /**
   * HttpHeaders class represents the header configuration options for an HTTP request.
   * @group Props
   */
  headers;
  /**
   * Whether to use the default upload or a manual implementation defined in uploadHandler callback. Defaults to PrimeNG Locale configuration.
   * @group Props
   */
  customUpload;
  /**
   * Maximum number of files that can be uploaded.
   * @group Props
   */
  fileLimit;
  /**
   * Style class of the upload button.
   * @group Props
   */
  uploadStyleClass;
  /**
   * Style class of the cancel button.
   * @group Props
   */
  cancelStyleClass;
  /**
   * Style class of the remove button.
   * @group Props
   */
  removeStyleClass;
  /**
   * Style class of the choose button.
   * @group Props
   */
  chooseStyleClass;
  /**
   * Used to pass all properties of the ButtonProps to the choose button inside the component.
   * @group Props
   */
  chooseButtonProps;
  /**
   * Used to pass all properties of the ButtonProps to the upload button inside the component.
   * @group Props
   */
  uploadButtonProps = {
    severity: "secondary"
  };
  /**
   * Used to pass all properties of the ButtonProps to the cancel button inside the component.
   * @group Props
   */
  cancelButtonProps = {
    severity: "secondary"
  };
  /**
   * Callback to invoke before file upload is initialized.
   * @param {FileBeforeUploadEvent} event - Custom upload event.
   * @group Emits
   */
  onBeforeUpload = new EventEmitter();
  /**
   * An event indicating that the request was sent to the server. Useful when a request may be retried multiple times, to distinguish between retries on the final event stream.
   * @param {FileSendEvent} event - Custom send event.
   * @group Emits
   */
  onSend = new EventEmitter();
  /**
   * Callback to invoke when file upload is complete.
   * @param {FileUploadEvent} event - Custom upload event.
   * @group Emits
   */
  onUpload = new EventEmitter();
  /**
   * Callback to invoke if file upload fails.
   * @param {FileUploadErrorEvent} event - Custom error event.
   * @group Emits
   */
  onError = new EventEmitter();
  /**
   * Callback to invoke when files in queue are removed without uploading using clear all button.
   * @param {Event} event - Browser event.
   * @group Emits
   */
  onClear = new EventEmitter();
  /**
   * Callback to invoke when a file is removed without uploading using clear button of a file.
   * @param {FileRemoveEvent} event - Remove event.
   * @group Emits
   */
  onRemove = new EventEmitter();
  /**
   * Callback to invoke when files are selected.
   * @param {FileSelectEvent} event - Select event.
   * @group Emits
   */
  onSelect = new EventEmitter();
  /**
   * Callback to invoke when files are being uploaded.
   * @param {FileProgressEvent} event - Progress event.
   * @group Emits
   */
  onProgress = new EventEmitter();
  /**
   * Callback to invoke in custom upload mode to upload the files manually.
   * @param {FileUploadHandlerEvent} event - Upload handler event.
   * @group Emits
   */
  uploadHandler = new EventEmitter();
  /**
   * This event is triggered if an error occurs while loading an image file.
   * @param {Event} event - Browser event.
   * @group Emits
   */
  onImageError = new EventEmitter();
  /**
   * This event is triggered if an error occurs while loading an image file.
   * @param {RemoveUploadedFileEvent} event - Remove event.
   * @group Emits
   */
  onRemoveUploadedFile = new EventEmitter();
  /**
   * Custom file template.
   * @group Templates
   */
  fileTemplate;
  /**
   * Custom header template.
   * @param {FileUploadHeaderTemplateContext} context - header template context.
   * @group Templates
   */
  headerTemplate;
  /**
   * Custom content template.
   * @param {FileUploadContentTemplateContext} context - content template context.
   * @group Templates
   */
  contentTemplate;
  /**
   * Custom toolbar template.
   * @group Templates
   */
  toolbarTemplate;
  /**
   * Custom choose icon template.
   * @group Templates
   */
  chooseIconTemplate;
  /**
   * Custom file label template.
   * @param {FileUploadFileLabelTemplateContext} context - file label template context.
   * @group Templates
   */
  fileLabelTemplate;
  /**
   * Custom upload icon template.
   * @group Templates
   */
  uploadIconTemplate;
  /**
   * Custom cancel icon template.
   * @group Templates
   */
  cancelIconTemplate;
  /**
   * Custom empty state template.
   * @group Templates
   */
  emptyTemplate;
  advancedFileInput;
  basicFileInput;
  content;
  set files(files) {
    this._files = [];
    for (let i = 0; i < files.length; i++) {
      let file = files[i];
      if (this.validate(file)) {
        if (this.isImage(file)) {
          file.objectURL = this.sanitizer.bypassSecurityTrustUrl(window.URL.createObjectURL(files[i]));
        }
        this._files.push(files[i]);
      }
    }
  }
  get files() {
    return this._files;
  }
  get basicButtonLabel() {
    if (this.auto || !this.hasFiles()) {
      return this.chooseLabel;
    }
    return this.uploadLabel ?? this.files[0].name;
  }
  _files = [];
  progress = 0;
  dragHighlight;
  msgs;
  uploadedFileCount = 0;
  focus;
  uploading;
  duplicateIEEvent;
  // flag to recognize duplicate onchange event for file input
  translationSubscription;
  dragOverListener;
  uploadedFiles = [];
  sanitizer = inject(DomSanitizer);
  zone = inject(NgZone);
  http = inject(HttpClient);
  _componentStyle = inject(FileUploadStyle);
  onInit() {
    this.translationSubscription = this.config.translationObserver.subscribe(() => {
      this.cd.markForCheck();
    });
  }
  onAfterViewInit() {
    if (isPlatformBrowser(this.platformId)) {
      if (this.mode === "advanced") {
        this.zone.runOutsideAngular(() => {
          if (this.content) {
            this.dragOverListener = this.renderer.listen(this.content.nativeElement, "dragover", this.onDragOver.bind(this));
          }
        });
      }
    }
  }
  _headerTemplate;
  _contentTemplate;
  _toolbarTemplate;
  _chooseIconTemplate;
  _uploadIconTemplate;
  _cancelIconTemplate;
  _emptyTemplate;
  _fileTemplate;
  _fileLabelTemplate;
  templates;
  onAfterContentInit() {
    this.templates?.forEach((item) => {
      switch (item.getType()) {
        case "header":
          this._headerTemplate = item.template;
          break;
        case "file":
          this._fileTemplate = item.template;
          break;
        case "content":
          this._contentTemplate = item.template;
          break;
        case "toolbar":
          this._toolbarTemplate = item.template;
          break;
        case "chooseicon":
          this._chooseIconTemplate = item.template;
          break;
        case "uploadicon":
          this._uploadIconTemplate = item.template;
          break;
        case "cancelicon":
          this._cancelIconTemplate = item.template;
          break;
        case "empty":
          this._emptyTemplate = item.template;
          break;
        case "filelabel":
          this._fileLabelTemplate = item.template;
          break;
        default:
          this._fileTemplate = item.template;
          break;
      }
    });
  }
  basicFileChosenLabel() {
    if (this.auto) return this.chooseButtonLabel;
    else if (this.hasFiles()) {
      if (this.files && this.files.length === 1) return this.files[0].name;
      return this.config.getTranslation("fileChosenMessage")?.replace("{0}", this.files.length);
    }
    return this.config.getTranslation("noFileChosenMessage") || "";
  }
  completedLabel() {
    return this.config.getTranslation("completed") || "";
  }
  getTranslation(option) {
    return this.config.getTranslation(option);
  }
  choose() {
    this.advancedFileInput?.nativeElement.click();
  }
  onFileSelect(event) {
    if (event.type !== "drop" && this.isIE11() && this.duplicateIEEvent) {
      this.duplicateIEEvent = false;
      return;
    }
    if (!this.multiple) {
      this.files = [];
    }
    this.msgs = [];
    this.files = this.files || [];
    let files = event.dataTransfer ? event.dataTransfer.files : event.target.files;
    for (let i = 0; i < files.length; i++) {
      let file = files[i];
      if (!this.isFileSelected(file)) {
        if (this.validate(file)) {
          if (this.isImage(file)) {
            file.objectURL = this.sanitizer.bypassSecurityTrustUrl(window.URL.createObjectURL(files[i]));
          }
          this.files.push(files[i]);
        }
      }
    }
    this.onSelect.emit({
      originalEvent: event,
      files,
      currentFiles: this.files
    });
    this.checkFileLimit(files);
    if (this.hasFiles() && this.auto && (this.mode !== "advanced" || !this.isFileLimitExceeded())) {
      this.upload();
    }
    if (event.type !== "drop" && this.isIE11()) {
      this.clearIEInput();
    } else {
      this.clearInputElement();
    }
  }
  isFileSelected(file) {
    for (let sFile of this.files) {
      if (sFile.name + sFile.type + sFile.size === file.name + file.type + file.size) {
        return true;
      }
    }
    return false;
  }
  isIE11() {
    if (isPlatformBrowser(this.platformId)) {
      return !!this.document.defaultView["MSInputMethodContext"] && !!this.document["documentMode"];
    }
  }
  validate(file) {
    this.msgs = this.msgs || [];
    if (this.accept && !this.isFileTypeValid(file)) {
      const text = `${this.invalidFileTypeMessageSummary.replace("{0}", file.name)} ${this.invalidFileTypeMessageDetail.replace("{0}", this.accept)}`;
      this.msgs.push({
        severity: "error",
        text
      });
      return false;
    }
    if (this.maxFileSize && file.size > this.maxFileSize) {
      const text = `${this.invalidFileSizeMessageSummary.replace("{0}", file.name)} ${this.invalidFileSizeMessageDetail.replace("{0}", this.formatSize(this.maxFileSize))}`;
      this.msgs.push({
        severity: "error",
        text
      });
      return false;
    }
    return true;
  }
  isFileTypeValid(file) {
    let acceptableTypes = this.accept?.split(",").map((type) => type.trim());
    for (let type of acceptableTypes) {
      let acceptable = this.isWildcard(type) ? this.getTypeClass(file.type) === this.getTypeClass(type) : file.type == type || this.getFileExtension(file).toLowerCase() === type.toLowerCase();
      if (acceptable) {
        return true;
      }
    }
    return false;
  }
  getTypeClass(fileType) {
    return fileType.substring(0, fileType.indexOf("/"));
  }
  isWildcard(fileType) {
    return fileType.indexOf("*") !== -1;
  }
  getFileExtension(file) {
    return "." + file.name.split(".").pop();
  }
  isImage(file) {
    return /^image\//.test(file.type);
  }
  onImageLoad(img) {
    window.URL.revokeObjectURL(img.src);
  }
  /**
   * Uploads the selected files.
   * @group Method
   */
  uploader() {
    if (this.customUpload) {
      if (this.fileLimit) {
        this.uploadedFileCount += this.files.length;
      }
      this.uploadHandler.emit({
        files: this.files
      });
      this.cd.markForCheck();
    } else {
      this.uploading = true;
      this.msgs = [];
      let formData = new FormData();
      this.onBeforeUpload.emit({
        formData
      });
      for (let i = 0; i < this.files.length; i++) {
        formData.append(this.name, this.files[i], this.files[i].name);
      }
      this.http.request(this.method, this.url, {
        body: formData,
        headers: this.headers,
        reportProgress: true,
        observe: "events",
        withCredentials: this.withCredentials
      }).subscribe((event) => {
        switch (event.type) {
          case HttpEventType.Sent:
            this.onSend.emit({
              originalEvent: event,
              formData
            });
            break;
          case HttpEventType.Response:
            this.uploading = false;
            this.progress = 0;
            if (event["status"] >= 200 && event["status"] < 300) {
              if (this.fileLimit) {
                this.uploadedFileCount += this.files.length;
              }
              this.onUpload.emit({
                originalEvent: event,
                files: this.files
              });
            } else {
              this.onError.emit({
                files: this.files
              });
            }
            this.uploadedFiles = [...this.uploadedFiles, ...this.files];
            this.clear();
            break;
          case HttpEventType.UploadProgress: {
            if (event["loaded"]) {
              this.progress = Math.round(event["loaded"] * 100 / event["total"]);
            }
            this.onProgress.emit({
              originalEvent: event,
              progress: this.progress
            });
            break;
          }
        }
        this.cd.markForCheck();
      }, (error) => {
        this.uploading = false;
        this.onError.emit({
          files: this.files,
          error
        });
      });
    }
  }
  onRemoveClick(e) {
    const {
      event,
      index
    } = e;
    if (this.hasFiles()) {
      this.remove(event, index);
    }
  }
  onRemoveUploadedFileClick(e) {
    const {
      index
    } = e;
    if (this.hasUploadedFiles()) {
      this.removeUploadedFile(index);
    }
  }
  /**
   * Clears the files list.
   * @group Method
   */
  clear() {
    this.files = [];
    this.onClear.emit();
    this.clearInputElement();
    this.msgs = [];
    this.cd.markForCheck();
  }
  /**
   * Removes a single file.
   * @param {Event} event - Browser event.
   * @param {Number} index - Index of the file.
   * @group Method
   */
  remove(event, index) {
    this.clearInputElement();
    this.onRemove.emit({
      originalEvent: event,
      file: this.files[index]
    });
    this.files.splice(index, 1);
    this.checkFileLimit(this.files);
  }
  /**
   * Removes uploaded file.
   * @param {Number} index - Index of the file to be removed.
   * @group Method
   */
  removeUploadedFile(index) {
    let removedFile = this.uploadedFiles.splice(index, 1)[0];
    this.uploadedFiles = [...this.uploadedFiles];
    this.onRemoveUploadedFile.emit({
      file: removedFile,
      files: this.uploadedFiles
    });
  }
  isFileLimitExceeded() {
    const isAutoMode = this.auto;
    const totalFileCount = isAutoMode ? this.files.length : this.files.length + this.uploadedFileCount;
    if (this.fileLimit && this.fileLimit <= totalFileCount && this.focus) {
      this.focus = false;
    }
    return this.fileLimit && this.fileLimit < totalFileCount;
  }
  isChooseDisabled() {
    if (this.auto) {
      return this.fileLimit && this.fileLimit <= this.files.length;
    } else {
      return this.fileLimit && this.fileLimit <= this.files.length + this.uploadedFileCount;
    }
  }
  checkFileLimit(files) {
    this.msgs ??= [];
    const hasExistingValidationMessages = this.msgs.length > 0 && this.fileLimit && this.fileLimit < files.length;
    if (this.isFileLimitExceeded() || hasExistingValidationMessages) {
      const text = `${this.invalidFileLimitMessageSummary.replace("{0}", this.fileLimit.toString())} ${this.invalidFileLimitMessageDetail.replace("{0}", this.fileLimit.toString())}`;
      this.msgs.push({
        severity: "error",
        text
      });
    } else {
      this.msgs = this.msgs.filter((msg) => !msg.text.includes(this.invalidFileLimitMessageSummary));
    }
  }
  clearInputElement() {
    if (this.advancedFileInput && this.advancedFileInput.nativeElement) {
      this.advancedFileInput.nativeElement.value = "";
    }
    if (this.basicFileInput && this.basicFileInput.nativeElement) {
      this.basicFileInput.nativeElement.value = "";
    }
  }
  clearIEInput() {
    if (this.advancedFileInput && this.advancedFileInput.nativeElement) {
      this.duplicateIEEvent = true;
      this.advancedFileInput.nativeElement.value = "";
    }
  }
  hasFiles() {
    return this.files && this.files.length > 0;
  }
  hasUploadedFiles() {
    return this.uploadedFiles && this.uploadedFiles.length > 0;
  }
  onDragEnter(e) {
    if (!this.disabled) {
      e.stopPropagation();
      e.preventDefault();
    }
  }
  onDragOver(e) {
    if (!this.disabled) {
      !this.$unstyled() && W(this.content?.nativeElement, "p-fileupload-highlight");
      this.content?.nativeElement.setAttribute("data-p-highlight", true);
      this.dragHighlight = true;
      e.stopPropagation();
      e.preventDefault();
    }
  }
  onDragLeave(event) {
    if (!this.disabled) {
      !this.$unstyled() && P(this.content?.nativeElement, "p-fileupload-highlight");
      this.content?.nativeElement.setAttribute("data-p-highlight", false);
    }
  }
  onDrop(event) {
    if (!this.disabled) {
      !this.$unstyled() && P(this.content?.nativeElement, "p-fileupload-highlight");
      this.content?.nativeElement.setAttribute("data-p-highlight", false);
      event.stopPropagation();
      event.preventDefault();
      let files = event.dataTransfer ? event.dataTransfer.files : event.target.files;
      let allowDrop = this.multiple || files && files.length === 1;
      if (allowDrop) {
        this.onFileSelect(event);
      }
    }
  }
  onFocus() {
    this.focus = true;
  }
  onBlur() {
    this.focus = false;
  }
  formatSize(bytes) {
    const k = 1024;
    const dm = 3;
    const sizes = this.getTranslation(TranslationKeys.FILE_SIZE_TYPES);
    if (bytes === 0) {
      return `0 ${sizes[0]}`;
    }
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    const formattedSize = (bytes / Math.pow(k, i)).toFixed(dm);
    return `${formattedSize} ${sizes[i]}`;
  }
  upload() {
    if (this.hasFiles()) this.uploader();
  }
  onBasicUploaderClick() {
    this.basicFileInput?.nativeElement.click();
  }
  onBasicKeydown(event) {
    switch (event.code) {
      case "Space":
      case "Enter":
        this.onBasicUploaderClick();
        event.preventDefault();
        break;
    }
  }
  imageError(event) {
    this.onImageError.emit(event);
  }
  getBlockableElement() {
    return this.el.nativeElement.children[0];
  }
  get chooseButtonLabel() {
    return this.chooseLabel || this.config.getTranslation(TranslationKeys.CHOOSE);
  }
  get uploadButtonLabel() {
    return this.uploadLabel || this.config.getTranslation(TranslationKeys.UPLOAD);
  }
  get cancelButtonLabel() {
    return this.cancelLabel || this.config.getTranslation(TranslationKeys.CANCEL);
  }
  get browseFilesLabel() {
    return this.config.getTranslation(TranslationKeys.ARIA)[TranslationKeys.BROWSE_FILES];
  }
  get pendingLabel() {
    return this.config.getTranslation(TranslationKeys.PENDING);
  }
  onDestroy() {
    if (this.content && this.content.nativeElement) {
      if (this.dragOverListener) {
        this.dragOverListener();
        this.dragOverListener = null;
      }
    }
    if (this.translationSubscription) {
      this.translationSubscription.unsubscribe();
    }
  }
  static ɵfac = /* @__PURE__ */ (() => {
    let ɵFileUpload_BaseFactory;
    return function FileUpload_Factory(__ngFactoryType__) {
      return (ɵFileUpload_BaseFactory || (ɵFileUpload_BaseFactory = ɵɵgetInheritedFactory(_FileUpload)))(__ngFactoryType__ || _FileUpload);
    };
  })();
  static ɵcmp = ɵɵdefineComponent({
    type: _FileUpload,
    selectors: [["p-fileupload"], ["p-fileUpload"]],
    contentQueries: function FileUpload_ContentQueries(rf, ctx, dirIndex) {
      if (rf & 1) {
        ɵɵcontentQuery(dirIndex, _c22, 4)(dirIndex, _c32, 4)(dirIndex, _c42, 4)(dirIndex, _c5, 4)(dirIndex, _c6, 4)(dirIndex, _c7, 4)(dirIndex, _c8, 4)(dirIndex, _c9, 4)(dirIndex, _c10, 4)(dirIndex, PrimeTemplate, 4);
      }
      if (rf & 2) {
        let _t;
        ɵɵqueryRefresh(_t = ɵɵloadQuery()) && (ctx.fileTemplate = _t.first);
        ɵɵqueryRefresh(_t = ɵɵloadQuery()) && (ctx.headerTemplate = _t.first);
        ɵɵqueryRefresh(_t = ɵɵloadQuery()) && (ctx.contentTemplate = _t.first);
        ɵɵqueryRefresh(_t = ɵɵloadQuery()) && (ctx.toolbarTemplate = _t.first);
        ɵɵqueryRefresh(_t = ɵɵloadQuery()) && (ctx.chooseIconTemplate = _t.first);
        ɵɵqueryRefresh(_t = ɵɵloadQuery()) && (ctx.fileLabelTemplate = _t.first);
        ɵɵqueryRefresh(_t = ɵɵloadQuery()) && (ctx.uploadIconTemplate = _t.first);
        ɵɵqueryRefresh(_t = ɵɵloadQuery()) && (ctx.cancelIconTemplate = _t.first);
        ɵɵqueryRefresh(_t = ɵɵloadQuery()) && (ctx.emptyTemplate = _t.first);
        ɵɵqueryRefresh(_t = ɵɵloadQuery()) && (ctx.templates = _t);
      }
    },
    viewQuery: function FileUpload_Query(rf, ctx) {
      if (rf & 1) {
        ɵɵviewQuery(_c11, 5)(_c122, 5)(_c42, 5);
      }
      if (rf & 2) {
        let _t;
        ɵɵqueryRefresh(_t = ɵɵloadQuery()) && (ctx.advancedFileInput = _t.first);
        ɵɵqueryRefresh(_t = ɵɵloadQuery()) && (ctx.basicFileInput = _t.first);
        ɵɵqueryRefresh(_t = ɵɵloadQuery()) && (ctx.content = _t.first);
      }
    },
    inputs: {
      name: "name",
      url: "url",
      method: "method",
      multiple: [2, "multiple", "multiple", booleanAttribute],
      accept: "accept",
      disabled: [2, "disabled", "disabled", booleanAttribute],
      auto: [2, "auto", "auto", booleanAttribute],
      withCredentials: [2, "withCredentials", "withCredentials", booleanAttribute],
      maxFileSize: [2, "maxFileSize", "maxFileSize", numberAttribute],
      invalidFileSizeMessageSummary: "invalidFileSizeMessageSummary",
      invalidFileSizeMessageDetail: "invalidFileSizeMessageDetail",
      invalidFileTypeMessageSummary: "invalidFileTypeMessageSummary",
      invalidFileTypeMessageDetail: "invalidFileTypeMessageDetail",
      invalidFileLimitMessageDetail: "invalidFileLimitMessageDetail",
      invalidFileLimitMessageSummary: "invalidFileLimitMessageSummary",
      style: "style",
      styleClass: "styleClass",
      previewWidth: [2, "previewWidth", "previewWidth", numberAttribute],
      chooseLabel: "chooseLabel",
      uploadLabel: "uploadLabel",
      cancelLabel: "cancelLabel",
      chooseIcon: "chooseIcon",
      uploadIcon: "uploadIcon",
      cancelIcon: "cancelIcon",
      showUploadButton: [2, "showUploadButton", "showUploadButton", booleanAttribute],
      showCancelButton: [2, "showCancelButton", "showCancelButton", booleanAttribute],
      mode: "mode",
      headers: "headers",
      customUpload: [2, "customUpload", "customUpload", booleanAttribute],
      fileLimit: [2, "fileLimit", "fileLimit", (value) => numberAttribute(value, void 0)],
      uploadStyleClass: "uploadStyleClass",
      cancelStyleClass: "cancelStyleClass",
      removeStyleClass: "removeStyleClass",
      chooseStyleClass: "chooseStyleClass",
      chooseButtonProps: "chooseButtonProps",
      uploadButtonProps: "uploadButtonProps",
      cancelButtonProps: "cancelButtonProps",
      files: "files"
    },
    outputs: {
      onBeforeUpload: "onBeforeUpload",
      onSend: "onSend",
      onUpload: "onUpload",
      onError: "onError",
      onClear: "onClear",
      onRemove: "onRemove",
      onSelect: "onSelect",
      onProgress: "onProgress",
      uploadHandler: "uploadHandler",
      onImageError: "onImageError",
      onRemoveUploadedFile: "onRemoveUploadedFile"
    },
    features: [ɵɵProvidersFeature([FileUploadStyle, {
      provide: FILEUPLOAD_INSTANCE,
      useExisting: _FileUpload
    }, {
      provide: PARENT_INSTANCE,
      useExisting: _FileUpload
    }]), ɵɵHostDirectivesFeature([Bind]), ɵɵInheritDefinitionFeature],
    decls: 2,
    vars: 2,
    consts: [["advancedfileinput", ""], ["content", ""], ["icon", ""], ["basicfileinput", ""], [3, "class", "ngStyle", "pBind", 4, "ngIf"], [3, "class", "pBind", 4, "ngIf"], [3, "ngStyle", "pBind"], ["type", "file", 3, "change", "multiple", "accept", "disabled", "pBind"], [3, "pBind"], [4, "ngIf"], [4, "ngTemplateOutlet", "ngTemplateOutletContext"], [4, "ngTemplateOutlet"], [3, "dragenter", "dragleave", "drop", "pBind"], [3, "focus", "blur", "onClick", "keydown.enter", "styleClass", "disabled", "label", "buttonProps", "pt", "unstyled"], [3, "label", "disabled", "styleClass", "buttonProps", "pt", "unstyled", "onClick", 4, "ngIf"], ["data-p-icon", "plus", 3, "pBind", 4, "ngIf"], [3, "pBind", 4, "ngIf"], ["data-p-icon", "plus", 3, "pBind"], [3, "onClick", "label", "disabled", "styleClass", "buttonProps", "pt", "unstyled"], [3, "ngClass", "pBind", 4, "ngIf"], [3, "ngClass", "pBind"], ["data-p-icon", "upload", 3, "pBind", 4, "ngIf"], ["data-p-icon", "upload", 3, "pBind"], [3, "ngClass", 4, "ngIf"], [3, "ngClass"], ["data-p-icon", "times", 4, "ngIf"], ["data-p-icon", "times"], [3, "value", "showValue", "pt", 4, "ngIf"], [3, "severity", "text", "pt", "unstyled"], [3, "class", "pBind"], [3, "value", "showValue", "pt"], ["ngFor", "", 3, "ngForOf", "ngForTemplate"], ["pFileContent", "", 3, "unstyled", "files", "badgeValue", "previewWidth", "fileRemoveIconTemplate"], ["pFileContent", "", 3, "onRemove", "unstyled", "files", "badgeValue", "previewWidth", "fileRemoveIconTemplate"], ["pFileContent", "", "badgeSeverity", "success", 3, "unstyled", "files", "badgeValue", "previewWidth", "fileRemoveIconTemplate"], ["pFileContent", "", "badgeSeverity", "success", 3, "onRemove", "unstyled", "files", "badgeValue", "previewWidth", "fileRemoveIconTemplate"], [3, "pBind", 4, "ngTemplateOutlet"], [3, "onClick", "keydown", "styleClass", "disabled", "label", "buttonProps", "pt", "unstyled"], ["type", "file", 3, "change", "focus", "blur", "accept", "multiple", "disabled", "pBind"], ["class", "p-button-icon p-button-icon-left", 3, "ngClass", "pBind", 4, "ngIf"], [1, "p-button-icon", "p-button-icon-left", 3, "ngClass", "pBind"], ["data-p-icon", "upload", 3, "class", "pBind", 4, "ngIf"], ["class", "p-button-icon p-button-icon-left", 3, "pBind", 4, "ngIf"], [1, "p-button-icon", "p-button-icon-left", 3, "pBind"], ["class", "p-button-icon p-button-icon-left pi", 3, "ngClass", "pBind", 4, "ngIf"], [1, "p-button-icon", "p-button-icon-left", "pi", 3, "ngClass", "pBind"]],
    template: function FileUpload_Template(rf, ctx) {
      if (rf & 1) {
        ɵɵtemplate(0, FileUpload_div_0_Template, 12, 28, "div", 4)(1, FileUpload_div_1_Template, 10, 20, "div", 5);
      }
      if (rf & 2) {
        ɵɵproperty("ngIf", ctx.mode === "advanced");
        ɵɵadvance();
        ɵɵproperty("ngIf", ctx.mode === "basic");
      }
    },
    dependencies: [CommonModule, NgClass, NgForOf, NgIf, NgTemplateOutlet, NgStyle, Button, ProgressBar, Message, PlusIcon, UploadIcon, TimesIcon, SharedModule, FileContent, Bind],
    encapsulation: 2,
    changeDetection: 0
  });
};
(() => {
  (typeof ngDevMode === "undefined" || ngDevMode) && setClassMetadata(FileUpload, [{
    type: Component,
    args: [{
      selector: "p-fileupload, p-fileUpload",
      standalone: true,
      imports: [CommonModule, Button, ProgressBar, Message, PlusIcon, UploadIcon, TimesIcon, SharedModule, FileContent, Bind],
      template: `
        <div [class]="cn(cx('root'), styleClass)" [ngStyle]="style" *ngIf="mode === 'advanced'" [pBind]="ptm('root')">
            <input [attr.aria-label]="browseFilesLabel" #advancedfileinput type="file" (change)="onFileSelect($event)" [multiple]="multiple" [accept]="accept" [disabled]="disabled || isChooseDisabled()" [attr.title]="''" [pBind]="ptm('input')" />
            <div [class]="cx('header')" [pBind]="ptm('header')">
                <ng-container *ngIf="!headerTemplate && !_headerTemplate">
                    <p-button
                        [styleClass]="cn(cx('pcChooseButton'), chooseStyleClass)"
                        [disabled]="disabled || isChooseDisabled()"
                        (focus)="onFocus()"
                        [label]="chooseButtonLabel"
                        (blur)="onBlur()"
                        (onClick)="choose()"
                        (keydown.enter)="choose()"
                        [buttonProps]="chooseButtonProps"
                        [pt]="ptm('pcChooseButton')"
                        [unstyled]="unstyled()"
                    >
                        <input
                            [attr.aria-label]="browseFilesLabel"
                            #advancedfileinput
                            type="file"
                            (change)="onFileSelect($event)"
                            [multiple]="multiple"
                            [accept]="accept"
                            [disabled]="disabled || isChooseDisabled()"
                            [attr.title]="''"
                            [pBind]="ptm('input')"
                        />
                        <ng-template #icon>
                            <span *ngIf="chooseIcon" [class]="chooseIcon" [attr.aria-label]="true" [pBind]="ptm('pcChooseButton')?.icon"></span>
                            <ng-container *ngIf="!chooseIcon">
                                <svg data-p-icon="plus" *ngIf="!chooseIconTemplate && !_chooseIconTemplate" [attr.aria-label]="true" [pBind]="ptm('pcChooseButton')?.icon" />
                                <span *ngIf="chooseIconTemplate || _chooseIconTemplate" [attr.aria-label]="true" [pBind]="ptm('pcChooseButton')?.icon">
                                    <ng-template *ngTemplateOutlet="chooseIconTemplate || _chooseIconTemplate"></ng-template>
                                </span>
                            </ng-container>
                        </ng-template>
                    </p-button>

                    <p-button
                        *ngIf="!auto && showUploadButton"
                        [label]="uploadButtonLabel"
                        (onClick)="upload()"
                        [disabled]="!hasFiles() || isFileLimitExceeded()"
                        [styleClass]="cn(cx('pcUploadButton'), uploadStyleClass)"
                        [buttonProps]="uploadButtonProps"
                        [pt]="ptm('pcUploadButton')"
                        [unstyled]="unstyled()"
                    >
                        <ng-template #icon>
                            <span *ngIf="uploadIcon" [ngClass]="uploadIcon" [attr.aria-hidden]="true" [pBind]="ptm('pcUploadButton')?.icon"></span>
                            <ng-container *ngIf="!uploadIcon">
                                <svg data-p-icon="upload" *ngIf="!uploadIconTemplate && !_uploadIconTemplate" [pBind]="ptm('pcUploadButton')?.icon" />
                                <span *ngIf="uploadIconTemplate || _uploadIconTemplate" [attr.aria-hidden]="true" [pBind]="ptm('pcUploadButton')?.icon">
                                    <ng-template *ngTemplateOutlet="uploadIconTemplate || _uploadIconTemplate"></ng-template>
                                </span>
                            </ng-container>
                        </ng-template>
                    </p-button>
                    <p-button
                        *ngIf="!auto && showCancelButton"
                        [label]="cancelButtonLabel"
                        (onClick)="clear()"
                        [disabled]="!hasFiles() || uploading"
                        [styleClass]="cn(cx('pcCancelButton'), cancelStyleClass)"
                        [buttonProps]="cancelButtonProps"
                        [pt]="ptm('pcCancelButton')"
                        [unstyled]="unstyled()"
                    >
                        <ng-template #icon>
                            <span *ngIf="cancelIcon" [ngClass]="cancelIcon"></span>
                            <ng-container *ngIf="!cancelIcon">
                                <svg data-p-icon="times" *ngIf="!cancelIconTemplate && !_cancelIconTemplate" [attr.aria-hidden]="true" />
                                <span *ngIf="cancelIconTemplate || _cancelIconTemplate" [attr.aria-hidden]="true">
                                    <ng-template *ngTemplateOutlet="cancelIconTemplate || _cancelIconTemplate"></ng-template>
                                </span>
                            </ng-container>
                        </ng-template>
                    </p-button>
                </ng-container>
                <ng-container
                    *ngTemplateOutlet="
                        headerTemplate || _headerTemplate;
                        context: {
                            $implicit: files,
                            uploadedFiles: uploadedFiles,
                            chooseCallback: choose.bind(this),
                            clearCallback: clear.bind(this),
                            uploadCallback: upload.bind(this)
                        }
                    "
                ></ng-container>
                <ng-container *ngTemplateOutlet="toolbarTemplate || _toolbarTemplate"></ng-container>
            </div>
            <div #content [class]="cx('content')" (dragenter)="onDragEnter($event)" (dragleave)="onDragLeave($event)" (drop)="onDrop($event)" [pBind]="ptm('content')">
                @if (contentTemplate || _contentTemplate) {
                    <ng-container
                        *ngTemplateOutlet="
                            contentTemplate || _contentTemplate;
                            context: {
                                $implicit: files,
                                uploadedFiles: uploadedFiles,
                                chooseCallback: choose.bind(this),
                                clearCallback: clear.bind(this),
                                removeUploadedFileCallback: removeUploadedFile.bind(this),
                                removeFileCallback: remove.bind(this),
                                progress: progress,
                                messages: msgs
                            }
                        "
                    ></ng-container>
                } @else {
                    <p-progressbar [value]="progress" [showValue]="false" *ngIf="hasFiles()" [pt]="ptm('pcProgressBar')"></p-progressbar>
                    @for (message of msgs; track message) {
                        <p-message [severity]="message.severity" [text]="message.text" [pt]="ptm('pcMessage')" [unstyled]="unstyled()"></p-message>
                    }

                    @if (hasFiles()) {
                        <div [class]="cx('fileList')" [pBind]="ptm('fileList')">
                            <ng-template ngFor [ngForOf]="files" [ngForTemplate]="fileTemplate || _fileTemplate"></ng-template>
                            @if (!fileTemplate && !_fileTemplate) {
                                <div
                                    pFileContent
                                    [unstyled]="unstyled()"
                                    [files]="files"
                                    (onRemove)="onRemoveClick($event)"
                                    [badgeValue]="pendingLabel"
                                    [previewWidth]="previewWidth"
                                    [fileRemoveIconTemplate]="cancelIconTemplate || _cancelIconTemplate"
                                ></div>
                            }
                        </div>
                    }
                    @if (hasUploadedFiles()) {
                        <div [class]="cx('fileList')" [pBind]="ptm('fileList')">
                            <ng-template ngFor [ngForOf]="uploadedFiles" [ngForTemplate]="fileTemplate || _fileTemplate"></ng-template>
                            @if (!fileTemplate && !_fileTemplate) {
                                <div
                                    pFileContent
                                    [unstyled]="unstyled()"
                                    [files]="uploadedFiles"
                                    (onRemove)="onRemoveUploadedFileClick($event)"
                                    [badgeValue]="completedLabel()"
                                    badgeSeverity="success"
                                    [previewWidth]="previewWidth"
                                    [fileRemoveIconTemplate]="cancelIconTemplate || _cancelIconTemplate"
                                ></div>
                            }
                        </div>
                    }
                }
                @if ((emptyTemplate || _emptyTemplate) && !hasFiles() && !hasUploadedFiles()) {
                    <ng-container *ngTemplateOutlet="emptyTemplate || _emptyTemplate" [pBind]="ptm('empty')"></ng-container>
                }
            </div>
        </div>
        <div [class]="cn(cx('root'), styleClass)" *ngIf="mode === 'basic'" [pBind]="ptm('root')">
            @for (message of msgs; track message) {
                <p-message [severity]="message.severity" [text]="message.text" [pt]="ptm('pcMessage')" [unstyled]="unstyled()"></p-message>
            }

            <div [class]="cx('basicContent')" [pBind]="ptm('basicContent')">
                <p-button
                    [styleClass]="cn(cx('pcChooseButton'), chooseStyleClass)"
                    [disabled]="disabled"
                    [label]="chooseButtonLabel"
                    [style]="style"
                    (onClick)="onBasicUploaderClick()"
                    (keydown)="onBasicKeydown($event)"
                    [buttonProps]="chooseButtonProps"
                    [pt]="ptm('pcChooseButton')"
                    [unstyled]="unstyled()"
                >
                    <ng-template #icon>
                        @if (hasFiles() && !auto) {
                            <span *ngIf="uploadIcon" class="p-button-icon p-button-icon-left" [ngClass]="uploadIcon" [pBind]="ptm('pcChooseButton')?.icon"></span>
                            <ng-container *ngIf="!uploadIcon">
                                <svg data-p-icon="upload" *ngIf="!uploadIconTemplate && !_uploadIconTemplate" [class]="'p-button-icon p-button-icon-left'" [pBind]="ptm('pcChooseButton')?.icon" />
                                <span *ngIf="_uploadIconTemplate || uploadIconTemplate" class="p-button-icon p-button-icon-left" [pBind]="ptm('pcChooseButton')?.icon">
                                    <ng-template *ngTemplateOutlet="_uploadIconTemplate || uploadIconTemplate"></ng-template>
                                </span>
                            </ng-container>
                        } @else {
                            <span *ngIf="chooseIcon" class="p-button-icon p-button-icon-left pi" [ngClass]="chooseIcon" [pBind]="ptm('pcChooseButton')?.icon"></span>
                            <ng-container *ngIf="!chooseIcon">
                                <svg data-p-icon="plus" *ngIf="!chooseIconTemplate && !_chooseIconTemplate" [pBind]="ptm('pcChooseButton')?.icon" />
                                <ng-template *ngTemplateOutlet="chooseIconTemplate || _chooseIconTemplate"></ng-template>
                            </ng-container>
                        }
                    </ng-template>
                    <input [attr.aria-label]="browseFilesLabel" #basicfileinput type="file" [accept]="accept" [multiple]="multiple" [disabled]="disabled" (change)="onFileSelect($event)" (focus)="onFocus()" (blur)="onBlur()" [pBind]="ptm('input')" />
                </p-button>
                @if (!auto) {
                    @if (!fileLabelTemplate && !_fileLabelTemplate) {
                        <span>
                            {{ basicFileChosenLabel() }}
                        </span>
                    } @else {
                        <ng-container *ngTemplateOutlet="fileLabelTemplate || _fileLabelTemplate; context: { $implicit: files }"></ng-container>
                    }
                }
            </div>
        </div>
    `,
      changeDetection: ChangeDetectionStrategy.OnPush,
      encapsulation: ViewEncapsulation.None,
      providers: [FileUploadStyle, {
        provide: FILEUPLOAD_INSTANCE,
        useExisting: FileUpload
      }, {
        provide: PARENT_INSTANCE,
        useExisting: FileUpload
      }],
      hostDirectives: [Bind]
    }]
  }], null, {
    name: [{
      type: Input
    }],
    url: [{
      type: Input
    }],
    method: [{
      type: Input
    }],
    multiple: [{
      type: Input,
      args: [{
        transform: booleanAttribute
      }]
    }],
    accept: [{
      type: Input
    }],
    disabled: [{
      type: Input,
      args: [{
        transform: booleanAttribute
      }]
    }],
    auto: [{
      type: Input,
      args: [{
        transform: booleanAttribute
      }]
    }],
    withCredentials: [{
      type: Input,
      args: [{
        transform: booleanAttribute
      }]
    }],
    maxFileSize: [{
      type: Input,
      args: [{
        transform: numberAttribute
      }]
    }],
    invalidFileSizeMessageSummary: [{
      type: Input
    }],
    invalidFileSizeMessageDetail: [{
      type: Input
    }],
    invalidFileTypeMessageSummary: [{
      type: Input
    }],
    invalidFileTypeMessageDetail: [{
      type: Input
    }],
    invalidFileLimitMessageDetail: [{
      type: Input
    }],
    invalidFileLimitMessageSummary: [{
      type: Input
    }],
    style: [{
      type: Input
    }],
    styleClass: [{
      type: Input
    }],
    previewWidth: [{
      type: Input,
      args: [{
        transform: numberAttribute
      }]
    }],
    chooseLabel: [{
      type: Input
    }],
    uploadLabel: [{
      type: Input
    }],
    cancelLabel: [{
      type: Input
    }],
    chooseIcon: [{
      type: Input
    }],
    uploadIcon: [{
      type: Input
    }],
    cancelIcon: [{
      type: Input
    }],
    showUploadButton: [{
      type: Input,
      args: [{
        transform: booleanAttribute
      }]
    }],
    showCancelButton: [{
      type: Input,
      args: [{
        transform: booleanAttribute
      }]
    }],
    mode: [{
      type: Input
    }],
    headers: [{
      type: Input
    }],
    customUpload: [{
      type: Input,
      args: [{
        transform: booleanAttribute
      }]
    }],
    fileLimit: [{
      type: Input,
      args: [{
        transform: (value) => numberAttribute(value, void 0)
      }]
    }],
    uploadStyleClass: [{
      type: Input
    }],
    cancelStyleClass: [{
      type: Input
    }],
    removeStyleClass: [{
      type: Input
    }],
    chooseStyleClass: [{
      type: Input
    }],
    chooseButtonProps: [{
      type: Input
    }],
    uploadButtonProps: [{
      type: Input
    }],
    cancelButtonProps: [{
      type: Input
    }],
    onBeforeUpload: [{
      type: Output
    }],
    onSend: [{
      type: Output
    }],
    onUpload: [{
      type: Output
    }],
    onError: [{
      type: Output
    }],
    onClear: [{
      type: Output
    }],
    onRemove: [{
      type: Output
    }],
    onSelect: [{
      type: Output
    }],
    onProgress: [{
      type: Output
    }],
    uploadHandler: [{
      type: Output
    }],
    onImageError: [{
      type: Output
    }],
    onRemoveUploadedFile: [{
      type: Output
    }],
    fileTemplate: [{
      type: ContentChild,
      args: ["file", {
        descendants: false
      }]
    }],
    headerTemplate: [{
      type: ContentChild,
      args: ["header", {
        descendants: false
      }]
    }],
    contentTemplate: [{
      type: ContentChild,
      args: ["content", {
        descendants: false
      }]
    }],
    toolbarTemplate: [{
      type: ContentChild,
      args: ["toolbar", {
        descendants: false
      }]
    }],
    chooseIconTemplate: [{
      type: ContentChild,
      args: ["chooseicon", {
        descendants: false
      }]
    }],
    fileLabelTemplate: [{
      type: ContentChild,
      args: ["filelabel", {
        descendants: false
      }]
    }],
    uploadIconTemplate: [{
      type: ContentChild,
      args: ["uploadicon", {
        descendants: false
      }]
    }],
    cancelIconTemplate: [{
      type: ContentChild,
      args: ["cancelicon", {
        descendants: false
      }]
    }],
    emptyTemplate: [{
      type: ContentChild,
      args: ["empty", {
        descendants: false
      }]
    }],
    advancedFileInput: [{
      type: ViewChild,
      args: ["advancedfileinput"]
    }],
    basicFileInput: [{
      type: ViewChild,
      args: ["basicfileinput"]
    }],
    content: [{
      type: ViewChild,
      args: ["content"]
    }],
    files: [{
      type: Input
    }],
    templates: [{
      type: ContentChildren,
      args: [PrimeTemplate]
    }]
  });
})();
var FileUploadModule = class _FileUploadModule {
  static ɵfac = function FileUploadModule_Factory(__ngFactoryType__) {
    return new (__ngFactoryType__ || _FileUploadModule)();
  };
  static ɵmod = ɵɵdefineNgModule({
    type: _FileUploadModule,
    imports: [FileUpload, SharedModule],
    exports: [FileUpload, SharedModule]
  });
  static ɵinj = ɵɵdefineInjector({
    imports: [FileUpload, SharedModule, SharedModule]
  });
};
(() => {
  (typeof ngDevMode === "undefined" || ngDevMode) && setClassMetadata(FileUploadModule, [{
    type: NgModule,
    args: [{
      imports: [FileUpload, SharedModule],
      exports: [FileUpload, SharedModule]
    }]
  }], null, null);
})();
export {
  FileContent,
  FileUpload,
  FileUploadClasses,
  FileUploadModule,
  FileUploadStyle
};
//# sourceMappingURL=primeng_fileupload.js.map
