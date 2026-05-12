import {
  MotionDirective,
  MotionModule
} from "./chunk-MEOAIN2L.js";
import {
  MinusIcon,
  PlusIcon
} from "./chunk-6O55ZLZ5.js";
import {
  BaseComponent,
  PARENT_INSTANCE
} from "./chunk-3OBHI6YD.js";
import {
  Bind,
  BindModule
} from "./chunk-HI3E3LQB.js";
import {
  BaseStyle
} from "./chunk-X3ZNKE5C.js";
import "./chunk-OBQZS5OM.js";
import {
  PrimeTemplate,
  SharedModule
} from "./chunk-GPM6AXFD.js";
import {
  CommonModule,
  NgIf,
  NgStyle,
  NgTemplateOutlet
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
  Output,
  ViewChild,
  ViewEncapsulation,
  booleanAttribute,
  computed,
  inject,
  input,
  setClassMetadata,
  ɵɵHostDirectivesFeature,
  ɵɵInheritDefinitionFeature,
  ɵɵProvidersFeature,
  ɵɵadvance,
  ɵɵattribute,
  ɵɵclassMap,
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
  ɵɵqueryRefresh,
  ɵɵreference,
  ɵɵresetView,
  ɵɵrestoreView,
  ɵɵtemplate,
  ɵɵtemplateRefExtractor,
  ɵɵtext,
  ɵɵtextInterpolate,
  ɵɵviewQuery
} from "./chunk-M2EDENCT.js";
import "./chunk-JRFR6BLO.js";
import "./chunk-HWYXSU2G.js";
import "./chunk-MARUHEWW.js";
import {
  s3 as s
} from "./chunk-7V5MERLO.js";
import {
  __spreadValues
} from "./chunk-GOMI4DH3.js";

// node_modules/@primeuix/styles/dist/fieldset/index.mjs
var style = "\n    .p-fieldset {\n        background: dt('fieldset.background');\n        border: 1px solid dt('fieldset.border.color');\n        border-radius: dt('fieldset.border.radius');\n        color: dt('fieldset.color');\n        padding: dt('fieldset.padding');\n        margin: 0;\n    }\n\n    .p-fieldset-legend {\n        background: dt('fieldset.legend.background');\n        border-radius: dt('fieldset.legend.border.radius');\n        border-width: dt('fieldset.legend.border.width');\n        border-style: solid;\n        border-color: dt('fieldset.legend.border.color');\n        color: dt('fieldset.legend.color');\n        padding: dt('fieldset.legend.padding');\n        transition:\n            background dt('fieldset.transition.duration'),\n            color dt('fieldset.transition.duration'),\n            outline-color dt('fieldset.transition.duration'),\n            box-shadow dt('fieldset.transition.duration');\n    }\n\n    .p-fieldset-toggleable > .p-fieldset-legend {\n        padding: 0;\n    }\n\n    .p-fieldset-toggle-button {\n        cursor: pointer;\n        user-select: none;\n        overflow: hidden;\n        position: relative;\n        text-decoration: none;\n        display: flex;\n        gap: dt('fieldset.legend.gap');\n        align-items: center;\n        justify-content: center;\n        padding: dt('fieldset.legend.padding');\n        background: transparent;\n        border: 0 none;\n        border-radius: dt('fieldset.legend.border.radius');\n        transition:\n            background dt('fieldset.transition.duration'),\n            color dt('fieldset.transition.duration'),\n            outline-color dt('fieldset.transition.duration'),\n            box-shadow dt('fieldset.transition.duration');\n        outline-color: transparent;\n    }\n\n    .p-fieldset-legend-label {\n        font-weight: dt('fieldset.legend.font.weight');\n    }\n\n    .p-fieldset-toggle-button:focus-visible {\n        box-shadow: dt('fieldset.legend.focus.ring.shadow');\n        outline: dt('fieldset.legend.focus.ring.width') dt('fieldset.legend.focus.ring.style') dt('fieldset.legend.focus.ring.color');\n        outline-offset: dt('fieldset.legend.focus.ring.offset');\n    }\n\n    .p-fieldset-toggleable > .p-fieldset-legend:hover {\n        color: dt('fieldset.legend.hover.color');\n        background: dt('fieldset.legend.hover.background');\n    }\n\n    .p-fieldset-toggle-icon {\n        color: dt('fieldset.toggle.icon.color');\n        transition: color dt('fieldset.transition.duration');\n    }\n\n    .p-fieldset-toggleable > .p-fieldset-legend:hover .p-fieldset-toggle-icon {\n        color: dt('fieldset.toggle.icon.hover.color');\n    }\n\n    .p-fieldset-content-container {\n        display: grid;\n        grid-template-rows: 1fr;\n    }\n\n    .p-fieldset-content-wrapper {\n        min-height: 0;\n    }\n\n    .p-fieldset-content {\n        padding: dt('fieldset.content.padding');\n    }\n";

// node_modules/primeng/fesm2022/primeng-fieldset.mjs
var _c0 = ["header"];
var _c1 = ["expandicon"];
var _c2 = ["collapseicon"];
var _c3 = ["content"];
var _c4 = ["contentWrapper"];
var _c5 = ["*", [["p-header"]]];
var _c6 = ["*", "p-header"];
function Fieldset_ng_container_2_ng_container_2__svg_svg_1_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵnamespaceSVG();
    ɵɵelement(0, "svg", 11);
  }
  if (rf & 2) {
    const ctx_r1 = ɵɵnextContext(3);
    ɵɵclassMap(ctx_r1.cx("toggleIcon"));
    ɵɵproperty("pBind", ctx_r1.ptm("toggleIcon"));
  }
}
function Fieldset_ng_container_2_ng_container_2_span_2_ng_container_1_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵelementContainer(0);
  }
}
function Fieldset_ng_container_2_ng_container_2_span_2_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵelementStart(0, "span", 3);
    ɵɵtemplate(1, Fieldset_ng_container_2_ng_container_2_span_2_ng_container_1_Template, 1, 0, "ng-container", 6);
    ɵɵelementEnd();
  }
  if (rf & 2) {
    const ctx_r1 = ɵɵnextContext(3);
    ɵɵclassMap(ctx_r1.cx("toggleIcon"));
    ɵɵproperty("pBind", ctx_r1.ptm("toggleIcon"));
    ɵɵadvance();
    ɵɵproperty("ngTemplateOutlet", ctx_r1.expandIconTemplate || ctx_r1._expandIconTemplate);
  }
}
function Fieldset_ng_container_2_ng_container_2_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵelementContainerStart(0);
    ɵɵtemplate(1, Fieldset_ng_container_2_ng_container_2__svg_svg_1_Template, 1, 3, "svg", 9)(2, Fieldset_ng_container_2_ng_container_2_span_2_Template, 2, 4, "span", 10);
    ɵɵelementContainerEnd();
  }
  if (rf & 2) {
    const ctx_r1 = ɵɵnextContext(2);
    ɵɵadvance();
    ɵɵproperty("ngIf", !ctx_r1.expandIconTemplate && !ctx_r1._expandIconTemplate);
    ɵɵadvance();
    ɵɵproperty("ngIf", ctx_r1.expandIconTemplate || ctx_r1._expandIconTemplate);
  }
}
function Fieldset_ng_container_2_ng_container_3__svg_svg_1_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵnamespaceSVG();
    ɵɵelement(0, "svg", 13);
  }
  if (rf & 2) {
    const ctx_r1 = ɵɵnextContext(3);
    ɵɵclassMap(ctx_r1.cx("toggleIcon"));
    ɵɵproperty("pBind", ctx_r1.ptm("toggleIcon"));
    ɵɵattribute("aria-hidden", true);
  }
}
function Fieldset_ng_container_2_ng_container_3_span_2_ng_container_1_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵelementContainer(0);
  }
}
function Fieldset_ng_container_2_ng_container_3_span_2_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵelementStart(0, "span", 3);
    ɵɵtemplate(1, Fieldset_ng_container_2_ng_container_3_span_2_ng_container_1_Template, 1, 0, "ng-container", 6);
    ɵɵelementEnd();
  }
  if (rf & 2) {
    const ctx_r1 = ɵɵnextContext(3);
    ɵɵclassMap(ctx_r1.cx("toggleIcon"));
    ɵɵproperty("pBind", ctx_r1.ptm("toggleIcon"));
    ɵɵadvance();
    ɵɵproperty("ngTemplateOutlet", ctx_r1.collapseIconTemplate || ctx_r1._collapseIconTemplate);
  }
}
function Fieldset_ng_container_2_ng_container_3_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵelementContainerStart(0);
    ɵɵtemplate(1, Fieldset_ng_container_2_ng_container_3__svg_svg_1_Template, 1, 4, "svg", 12)(2, Fieldset_ng_container_2_ng_container_3_span_2_Template, 2, 4, "span", 10);
    ɵɵelementContainerEnd();
  }
  if (rf & 2) {
    const ctx_r1 = ɵɵnextContext(2);
    ɵɵadvance();
    ɵɵproperty("ngIf", !ctx_r1.collapseIconTemplate && !ctx_r1._collapseIconTemplate);
    ɵɵadvance();
    ɵɵproperty("ngIf", ctx_r1.collapseIconTemplate || ctx_r1._collapseIconTemplate);
  }
}
function Fieldset_ng_container_2_ng_container_4_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵelementContainer(0);
  }
}
function Fieldset_ng_container_2_Template(rf, ctx) {
  if (rf & 1) {
    const _r1 = ɵɵgetCurrentView();
    ɵɵelementContainerStart(0);
    ɵɵelementStart(1, "button", 7);
    ɵɵlistener("click", function Fieldset_ng_container_2_Template_button_click_1_listener($event) {
      ɵɵrestoreView(_r1);
      const ctx_r1 = ɵɵnextContext();
      return ɵɵresetView(ctx_r1.toggle($event));
    })("keydown", function Fieldset_ng_container_2_Template_button_keydown_1_listener($event) {
      ɵɵrestoreView(_r1);
      const ctx_r1 = ɵɵnextContext();
      return ɵɵresetView(ctx_r1.onKeyDown($event));
    });
    ɵɵtemplate(2, Fieldset_ng_container_2_ng_container_2_Template, 3, 2, "ng-container", 8)(3, Fieldset_ng_container_2_ng_container_3_Template, 3, 2, "ng-container", 8)(4, Fieldset_ng_container_2_ng_container_4_Template, 1, 0, "ng-container", 6);
    ɵɵelementEnd();
    ɵɵelementContainerEnd();
  }
  if (rf & 2) {
    const ctx_r1 = ɵɵnextContext();
    const legendContent_r3 = ɵɵreference(4);
    ɵɵadvance();
    ɵɵclassMap(ctx_r1.cx("toggleButton"));
    ɵɵproperty("pBind", ctx_r1.ptm("toggleButton"));
    ɵɵattribute("id", ctx_r1.id + "_header")("aria-controls", ctx_r1.id + "_content")("aria-expanded", !ctx_r1.collapsed)("aria-label", ctx_r1.buttonAriaLabel);
    ɵɵadvance();
    ɵɵproperty("ngIf", ctx_r1.collapsed);
    ɵɵadvance();
    ɵɵproperty("ngIf", !ctx_r1.collapsed);
    ɵɵadvance();
    ɵɵproperty("ngTemplateOutlet", legendContent_r3);
  }
}
function Fieldset_ng_template_3_ng_container_3_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵelementContainer(0);
  }
}
function Fieldset_ng_template_3_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵelementStart(0, "span", 3);
    ɵɵtext(1);
    ɵɵelementEnd();
    ɵɵprojection(2, 1);
    ɵɵtemplate(3, Fieldset_ng_template_3_ng_container_3_Template, 1, 0, "ng-container", 6);
  }
  if (rf & 2) {
    const ctx_r1 = ɵɵnextContext();
    ɵɵclassMap(ctx_r1.cx("legendLabel"));
    ɵɵproperty("pBind", ctx_r1.ptm("legendLabel"));
    ɵɵadvance();
    ɵɵtextInterpolate(ctx_r1.legend);
    ɵɵadvance(2);
    ɵɵproperty("ngTemplateOutlet", ctx_r1.headerTemplate || ctx_r1._headerTemplate);
  }
}
function Fieldset_ng_container_10_Template(rf, ctx) {
  if (rf & 1) {
    ɵɵelementContainer(0);
  }
}
var classes = {
  root: ({
    instance
  }) => ["p-fieldset p-component", {
    "p-fieldset-toggleable": instance.toggleable,
    "p-fieldset-collapsed": instance.collapsed && instance.toggleable
  }],
  legend: "p-fieldset-legend",
  legendLabel: "p-fieldset-legend-label",
  toggleButton: "p-fieldset-toggle-button",
  toggleIcon: "p-fieldset-toggle-icon",
  contentContainer: "p-fieldset-content-container",
  contentWrapper: "p-fieldset-content-wrapper",
  content: "p-fieldset-content"
};
var FieldsetStyle = class _FieldsetStyle extends BaseStyle {
  name = "fieldset";
  style = style;
  classes = classes;
  static ɵfac = /* @__PURE__ */ (() => {
    let ɵFieldsetStyle_BaseFactory;
    return function FieldsetStyle_Factory(__ngFactoryType__) {
      return (ɵFieldsetStyle_BaseFactory || (ɵFieldsetStyle_BaseFactory = ɵɵgetInheritedFactory(_FieldsetStyle)))(__ngFactoryType__ || _FieldsetStyle);
    };
  })();
  static ɵprov = ɵɵdefineInjectable({
    token: _FieldsetStyle,
    factory: _FieldsetStyle.ɵfac
  });
};
(() => {
  (typeof ngDevMode === "undefined" || ngDevMode) && setClassMetadata(FieldsetStyle, [{
    type: Injectable
  }], null, null);
})();
var FieldsetClasses;
(function(FieldsetClasses2) {
  FieldsetClasses2["root"] = "p-fieldset";
  FieldsetClasses2["legend"] = "p-fieldset-legend";
  FieldsetClasses2["legendLabel"] = "p-fieldset-legend-label";
  FieldsetClasses2["toggleIcon"] = "p-fieldset-toggle-icon";
  FieldsetClasses2["contentContainer"] = "p-fieldset-content-container";
  FieldsetClasses2["contentWrapper"] = "p-fieldset-content-wrapper";
  FieldsetClasses2["content"] = "p-fieldset-content";
})(FieldsetClasses || (FieldsetClasses = {}));
var FIELDSET_INSTANCE = new InjectionToken("FIELDSET_INSTANCE");
var Fieldset = class _Fieldset extends BaseComponent {
  componentName = "Fieldset";
  $pcFieldset = inject(FIELDSET_INSTANCE, {
    optional: true,
    skipSelf: true
  }) ?? void 0;
  _componentStyle = inject(FieldsetStyle);
  bindDirectiveInstance = inject(Bind, {
    self: true
  });
  onAfterViewChecked() {
    this.bindDirectiveInstance.setAttrs(this.ptm("host"));
  }
  get dataP() {
    return this.cn({
      toggleable: this.toggleable
    });
  }
  /**
   * Header text of the fieldset.
   * @group Props
   */
  legend;
  /**
   * When specified, content can toggled by clicking the legend.
   * @group Props
   * @defaultValue false
   */
  toggleable;
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
   * Transition options of the panel animation.
   * @group Props
   * @deprecated since v21.0.0, use `motionOptions` instead.
   */
  transitionOptions = "400ms cubic-bezier(0.86, 0, 0.07, 1)";
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
   * Emits when the collapsed state changes.
   * @param {boolean} value - New value.
   * @group Emits
   */
  collapsedChange = new EventEmitter();
  /**
   * Callback to invoke before panel toggle.
   * @param {PanelBeforeToggleEvent} event - Custom toggle event
   * @group Emits
   */
  onBeforeToggle = new EventEmitter();
  /**
   * Callback to invoke after panel toggle.
   * @param {PanelAfterToggleEvent} event - Custom toggle event
   * @group Emits
   */
  onAfterToggle = new EventEmitter();
  contentWrapperViewChild;
  _id = s("pn_id_");
  get id() {
    return this._id;
  }
  get buttonAriaLabel() {
    return this.legend;
  }
  /**
   * Internal collapsed state
   */
  _collapsed;
  /**
   * Defines the initial state of content, supports one or two-way binding as well.
   * @group Props
   */
  get collapsed() {
    return this._collapsed;
  }
  set collapsed(value) {
    this._collapsed = value;
  }
  /**
   * Custom header template.
   * @group Templates
   */
  headerTemplate;
  /**
   * Custom expand icon template.
   * @group Templates
   */
  expandIconTemplate;
  /**
   * Custom collapse icon template.
   * @group Templates
   */
  collapseIconTemplate;
  /**
   * Custom content template.
   * @group Templates
   */
  contentTemplate;
  toggle(event) {
    this.onBeforeToggle.emit({
      originalEvent: event,
      collapsed: this.collapsed
    });
    if (this.collapsed) this.expand();
    else this.collapse();
    event.preventDefault();
  }
  onKeyDown(event) {
    if (event.code === "Enter" || event.code === "Space") {
      this.toggle(event);
      event.preventDefault();
    }
  }
  expand() {
    this._collapsed = false;
    this.collapsedChange.emit(false);
    this.updateTabIndex();
  }
  collapse() {
    this._collapsed = true;
    this.collapsedChange.emit(true);
    this.updateTabIndex();
  }
  getBlockableElement() {
    return this.el.nativeElement.children[0];
  }
  updateTabIndex() {
    if (this.contentWrapperViewChild) {
      const focusableElements = this.contentWrapperViewChild.nativeElement.querySelectorAll("input, button, select, a, textarea, [tabindex]");
      focusableElements.forEach((element) => {
        if (this.collapsed) {
          element.setAttribute("tabindex", "-1");
        } else {
          element.removeAttribute("tabindex");
        }
      });
    }
  }
  onToggleDone(event) {
    this.onAfterToggle.emit({
      originalEvent: event,
      collapsed: this.collapsed
    });
  }
  _headerTemplate;
  _expandIconTemplate;
  _collapseIconTemplate;
  _contentTemplate;
  templates;
  onAfterContentInit() {
    this.templates.forEach((item) => {
      switch (item.getType()) {
        case "header":
          this._headerTemplate = item.template;
          break;
        case "expandicon":
          this._expandIconTemplate = item.template;
          break;
        case "collapseicon":
          this._collapseIconTemplate = item.template;
          break;
        case "content":
          this._contentTemplate = item.template;
          break;
      }
    });
  }
  static ɵfac = /* @__PURE__ */ (() => {
    let ɵFieldset_BaseFactory;
    return function Fieldset_Factory(__ngFactoryType__) {
      return (ɵFieldset_BaseFactory || (ɵFieldset_BaseFactory = ɵɵgetInheritedFactory(_Fieldset)))(__ngFactoryType__ || _Fieldset);
    };
  })();
  static ɵcmp = ɵɵdefineComponent({
    type: _Fieldset,
    selectors: [["p-fieldset"]],
    contentQueries: function Fieldset_ContentQueries(rf, ctx, dirIndex) {
      if (rf & 1) {
        ɵɵcontentQuery(dirIndex, _c0, 4)(dirIndex, _c1, 4)(dirIndex, _c2, 4)(dirIndex, _c3, 4)(dirIndex, PrimeTemplate, 4);
      }
      if (rf & 2) {
        let _t;
        ɵɵqueryRefresh(_t = ɵɵloadQuery()) && (ctx.headerTemplate = _t.first);
        ɵɵqueryRefresh(_t = ɵɵloadQuery()) && (ctx.expandIconTemplate = _t.first);
        ɵɵqueryRefresh(_t = ɵɵloadQuery()) && (ctx.collapseIconTemplate = _t.first);
        ɵɵqueryRefresh(_t = ɵɵloadQuery()) && (ctx.contentTemplate = _t.first);
        ɵɵqueryRefresh(_t = ɵɵloadQuery()) && (ctx.templates = _t);
      }
    },
    viewQuery: function Fieldset_Query(rf, ctx) {
      if (rf & 1) {
        ɵɵviewQuery(_c4, 5);
      }
      if (rf & 2) {
        let _t;
        ɵɵqueryRefresh(_t = ɵɵloadQuery()) && (ctx.contentWrapperViewChild = _t.first);
      }
    },
    inputs: {
      legend: "legend",
      toggleable: [2, "toggleable", "toggleable", booleanAttribute],
      style: "style",
      styleClass: "styleClass",
      transitionOptions: "transitionOptions",
      motionOptions: [1, "motionOptions"],
      collapsed: [2, "collapsed", "collapsed", booleanAttribute]
    },
    outputs: {
      collapsedChange: "collapsedChange",
      onBeforeToggle: "onBeforeToggle",
      onAfterToggle: "onAfterToggle"
    },
    features: [ɵɵProvidersFeature([FieldsetStyle, {
      provide: FIELDSET_INSTANCE,
      useExisting: _Fieldset
    }, {
      provide: PARENT_INSTANCE,
      useExisting: _Fieldset
    }]), ɵɵHostDirectivesFeature([Bind]), ɵɵInheritDefinitionFeature],
    ngContentSelectors: _c6,
    decls: 11,
    vars: 28,
    consts: [["legendContent", ""], ["contentWrapper", ""], [3, "ngStyle", "pBind"], [3, "pBind"], [4, "ngIf", "ngIfElse"], ["pMotionName", "p-collapsible", "role", "region", 3, "pMotionOnAfterEnter", "pMotionOnAfterLeave", "pBind", "pMotion", "pMotionOptions", "id"], [4, "ngTemplateOutlet"], ["tabindex", "0", "role", "button", 3, "click", "keydown", "pBind"], [4, "ngIf"], ["data-p-icon", "plus", 3, "class", "pBind", 4, "ngIf"], [3, "class", "pBind", 4, "ngIf"], ["data-p-icon", "plus", 3, "pBind"], ["data-p-icon", "minus", 3, "class", "pBind", 4, "ngIf"], ["data-p-icon", "minus", 3, "pBind"]],
    template: function Fieldset_Template(rf, ctx) {
      if (rf & 1) {
        ɵɵprojectionDef(_c5);
        ɵɵelementStart(0, "fieldset", 2)(1, "legend", 3);
        ɵɵtemplate(2, Fieldset_ng_container_2_Template, 5, 10, "ng-container", 4)(3, Fieldset_ng_template_3_Template, 4, 5, "ng-template", null, 0, ɵɵtemplateRefExtractor);
        ɵɵelementEnd();
        ɵɵelementStart(5, "div", 5);
        ɵɵlistener("pMotionOnAfterEnter", function Fieldset_Template_div_pMotionOnAfterEnter_5_listener($event) {
          return ctx.onToggleDone($event);
        })("pMotionOnAfterLeave", function Fieldset_Template_div_pMotionOnAfterLeave_5_listener($event) {
          return ctx.onToggleDone($event);
        });
        ɵɵelementStart(6, "div", 3)(7, "div", 3, 1);
        ɵɵprojection(9);
        ɵɵtemplate(10, Fieldset_ng_container_10_Template, 1, 0, "ng-container", 6);
        ɵɵelementEnd()()()();
      }
      if (rf & 2) {
        const legendContent_r3 = ɵɵreference(4);
        ɵɵclassMap(ctx.cn(ctx.cx("root"), ctx.styleClass));
        ɵɵproperty("ngStyle", ctx.style)("pBind", ctx.ptm("root"));
        ɵɵattribute("id", ctx.id)("data-p", ctx.dataP);
        ɵɵadvance();
        ɵɵclassMap(ctx.cx("legend"));
        ɵɵproperty("pBind", ctx.ptm("legend"));
        ɵɵattribute("data-p", ctx.dataP);
        ɵɵadvance();
        ɵɵproperty("ngIf", ctx.toggleable)("ngIfElse", legendContent_r3);
        ɵɵadvance(3);
        ɵɵclassMap(ctx.cx("contentContainer"));
        ɵɵproperty("pBind", ctx.ptm("contentContainer"))("pMotion", !ctx.toggleable || ctx.toggleable && !ctx.collapsed)("pMotionOptions", ctx.computedMotionOptions())("id", ctx.id + "_content");
        ɵɵattribute("aria-labelledby", ctx.id + "_header")("aria-hidden", ctx.collapsed)("tabindex", ctx.collapsed ? "-1" : void 0);
        ɵɵadvance();
        ɵɵclassMap(ctx.cx("contentWrapper"));
        ɵɵproperty("pBind", ctx.ptm("contentWrapper"));
        ɵɵadvance();
        ɵɵclassMap(ctx.cx("content"));
        ɵɵproperty("pBind", ctx.ptm("content"));
        ɵɵadvance(3);
        ɵɵproperty("ngTemplateOutlet", ctx.contentTemplate || ctx._contentTemplate);
      }
    },
    dependencies: [CommonModule, NgIf, NgTemplateOutlet, NgStyle, MinusIcon, PlusIcon, SharedModule, BindModule, Bind, MotionModule, MotionDirective],
    encapsulation: 2,
    changeDetection: 0
  });
};
(() => {
  (typeof ngDevMode === "undefined" || ngDevMode) && setClassMetadata(Fieldset, [{
    type: Component,
    args: [{
      selector: "p-fieldset",
      standalone: true,
      imports: [CommonModule, MinusIcon, PlusIcon, SharedModule, BindModule, MotionModule],
      template: `
        <fieldset [attr.id]="id" [ngStyle]="style" [class]="cn(cx('root'), styleClass)" [pBind]="ptm('root')" [attr.data-p]="dataP">
            <legend [class]="cx('legend')" [pBind]="ptm('legend')" [attr.data-p]="dataP">
                <ng-container *ngIf="toggleable; else legendContent">
                    <button
                        [attr.id]="id + '_header'"
                        tabindex="0"
                        role="button"
                        [attr.aria-controls]="id + '_content'"
                        [attr.aria-expanded]="!collapsed"
                        [attr.aria-label]="buttonAriaLabel"
                        (click)="toggle($event)"
                        (keydown)="onKeyDown($event)"
                        [class]="cx('toggleButton')"
                        [pBind]="ptm('toggleButton')"
                    >
                        <ng-container *ngIf="collapsed">
                            <svg data-p-icon="plus" *ngIf="!expandIconTemplate && !_expandIconTemplate" [class]="cx('toggleIcon')" [pBind]="ptm('toggleIcon')" />
                            <span *ngIf="expandIconTemplate || _expandIconTemplate" [class]="cx('toggleIcon')" [pBind]="ptm('toggleIcon')">
                                <ng-container *ngTemplateOutlet="expandIconTemplate || _expandIconTemplate"></ng-container>
                            </span>
                        </ng-container>
                        <ng-container *ngIf="!collapsed">
                            <svg data-p-icon="minus" *ngIf="!collapseIconTemplate && !_collapseIconTemplate" [class]="cx('toggleIcon')" [attr.aria-hidden]="true" [pBind]="ptm('toggleIcon')" />
                            <span *ngIf="collapseIconTemplate || _collapseIconTemplate" [class]="cx('toggleIcon')" [pBind]="ptm('toggleIcon')">
                                <ng-container *ngTemplateOutlet="collapseIconTemplate || _collapseIconTemplate"></ng-container>
                            </span>
                        </ng-container>
                        <ng-container *ngTemplateOutlet="legendContent"></ng-container>
                    </button>
                </ng-container>
                <ng-template #legendContent>
                    <span [class]="cx('legendLabel')" [pBind]="ptm('legendLabel')">{{ legend }}</span>
                    <ng-content select="p-header"></ng-content>
                    <ng-container *ngTemplateOutlet="headerTemplate || _headerTemplate"></ng-container>
                </ng-template>
            </legend>
            <div
                [pBind]="ptm('contentContainer')"
                [pMotion]="!toggleable || (toggleable && !collapsed)"
                pMotionName="p-collapsible"
                [pMotionOptions]="computedMotionOptions()"
                [class]="cx('contentContainer')"
                [id]="id + '_content'"
                role="region"
                [attr.aria-labelledby]="id + '_header'"
                [attr.aria-hidden]="collapsed"
                [attr.tabindex]="collapsed ? '-1' : undefined"
                (pMotionOnAfterEnter)="onToggleDone($event)"
                (pMotionOnAfterLeave)="onToggleDone($event)"
            >
                <div [pBind]="ptm('contentWrapper')" [class]="cx('contentWrapper')">
                    <div [class]="cx('content')" [pBind]="ptm('content')" #contentWrapper>
                        <ng-content></ng-content>
                        <ng-container *ngTemplateOutlet="contentTemplate || _contentTemplate"></ng-container>
                    </div>
                </div>
            </div>
        </fieldset>
    `,
      changeDetection: ChangeDetectionStrategy.OnPush,
      encapsulation: ViewEncapsulation.None,
      providers: [FieldsetStyle, {
        provide: FIELDSET_INSTANCE,
        useExisting: Fieldset
      }, {
        provide: PARENT_INSTANCE,
        useExisting: Fieldset
      }],
      hostDirectives: [Bind]
    }]
  }], null, {
    legend: [{
      type: Input
    }],
    toggleable: [{
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
    transitionOptions: [{
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
    collapsedChange: [{
      type: Output
    }],
    onBeforeToggle: [{
      type: Output
    }],
    onAfterToggle: [{
      type: Output
    }],
    contentWrapperViewChild: [{
      type: ViewChild,
      args: ["contentWrapper"]
    }],
    collapsed: [{
      type: Input,
      args: [{
        transform: booleanAttribute
      }]
    }],
    headerTemplate: [{
      type: ContentChild,
      args: ["header", {
        descendants: false
      }]
    }],
    expandIconTemplate: [{
      type: ContentChild,
      args: ["expandicon", {
        descendants: false
      }]
    }],
    collapseIconTemplate: [{
      type: ContentChild,
      args: ["collapseicon", {
        descendants: false
      }]
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
var FieldsetModule = class _FieldsetModule {
  static ɵfac = function FieldsetModule_Factory(__ngFactoryType__) {
    return new (__ngFactoryType__ || _FieldsetModule)();
  };
  static ɵmod = ɵɵdefineNgModule({
    type: _FieldsetModule,
    imports: [Fieldset, SharedModule, BindModule],
    exports: [Fieldset, SharedModule, BindModule]
  });
  static ɵinj = ɵɵdefineInjector({
    imports: [Fieldset, SharedModule, BindModule, SharedModule, BindModule]
  });
};
(() => {
  (typeof ngDevMode === "undefined" || ngDevMode) && setClassMetadata(FieldsetModule, [{
    type: NgModule,
    args: [{
      imports: [Fieldset, SharedModule, BindModule],
      exports: [Fieldset, SharedModule, BindModule]
    }]
  }], null, null);
})();
export {
  Fieldset,
  FieldsetClasses,
  FieldsetModule,
  FieldsetStyle
};
//# sourceMappingURL=primeng_fieldset.js.map
