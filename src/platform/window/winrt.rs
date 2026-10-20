//! Every WinRT and COM interface the window calls, with the vtable slots it uses. A GUID line ends
//! with `// <SDK header> <interface>`, a slot line with `// <interface>.<method>`;
//! `tools/verify-abi.ps1` reads both and compares them with the installed SDK.

use crate::platform::abi::Guid;

// XAML Islands.
pub const XAML_MANAGER_STATICS: Guid = Guid::from_u128(0x28258a12_7d82_505b_b210_712b04a58882); // windows.ui.xaml.hosting.h IWindowsXamlManagerStatics
pub const XAML_MANAGER_INITIALIZE: usize = 6; // IWindowsXamlManagerStatics.InitializeForCurrentThread
pub const CLOSABLE: Guid = Guid::from_u128(0x30d5a829_7fa4_4026_83bb_d75bae4ea99e); // windows.foundation.h IClosable
pub const CLOSABLE_CLOSE: usize = 6; // IClosable.Close
pub const XAML_SOURCE: Guid = Guid::from_u128(0xd585bfe1_00ff_51be_ba1d_a1329956ea0a); // windows.ui.xaml.hosting.h IDesktopWindowXamlSource
pub const XAML_SOURCE_PUT_CONTENT: usize = 7; // IDesktopWindowXamlSource.put_Content
pub const XAML_SOURCE_NATIVE: Guid = Guid::from_u128(0x3cbcf1bf_2f76_4e9c_96ab_e84b37972554); // windows.ui.xaml.hosting.desktopwindowxamlsource.h IDesktopWindowXamlSourceNative
pub const XAML_SOURCE_ATTACH: usize = 3; // IDesktopWindowXamlSourceNative.AttachToWindow
pub const XAML_SOURCE_WINDOW: usize = 4; // IDesktopWindowXamlSourceNative.get_WindowHandle
pub const XAML_SOURCE_NATIVE2: Guid = Guid::from_u128(0xe3dcd8c7_3057_4692_99c3_7b7720afda31); // windows.ui.xaml.hosting.desktopwindowxamlsource.h IDesktopWindowXamlSourceNative2
pub const XAML_SOURCE_PRETRANSLATE: usize = 5; // IDesktopWindowXamlSourceNative2.PreTranslateMessage
pub const XAML_READER_STATICS: Guid = Guid::from_u128(0x9891c6bd_534f_4955_b85a_8a8dc0dca602); // windows.ui.xaml.markup.h IXamlReaderStatics
pub const XAML_READER_LOAD: usize = 6; // IXamlReaderStatics.Load

// Elements.
pub const INSPECTABLE_CLASS_NAME: usize = 4; // IInspectable.GetRuntimeClassName
pub const UI_ELEMENT: Guid = Guid::from_u128(0x676d0be9_b65c_41c6_ba40_58cf87f201c1); // windows.ui.xaml.h IUIElement
pub const UI_ELEMENT_PUT_OPACITY: usize = 10; // IUIElement.put_Opacity
pub const UI_ELEMENT_TRANSFORM_TO_VISUAL: usize = 98; // IUIElement.TransformToVisual
pub const UI_ELEMENT_UPDATE_LAYOUT: usize = 101; // IUIElement.UpdateLayout
pub const UI_ELEMENT_ADD_KEY_DOWN: usize = 43; // IUIElement.add_KeyDown
pub const UI_ELEMENT_REMOVE_KEY_DOWN: usize = 44; // IUIElement.remove_KeyDown
pub const UI_ELEMENT_ADD_GOT_FOCUS: usize = 45; // IUIElement.add_GotFocus
pub const UI_ELEMENT_REMOVE_GOT_FOCUS: usize = 46; // IUIElement.remove_GotFocus
pub const UI_ELEMENT_ADD_LOST_FOCUS: usize = 47; // IUIElement.add_LostFocus
pub const UI_ELEMENT_REMOVE_LOST_FOCUS: usize = 48; // IUIElement.remove_LostFocus
pub const UI_ELEMENT_ADD_DRAG_ENTER: usize = 49; // IUIElement.add_DragEnter
pub const UI_ELEMENT_REMOVE_DRAG_ENTER: usize = 50; // IUIElement.remove_DragEnter
pub const UI_ELEMENT_ADD_DRAG_LEAVE: usize = 51; // IUIElement.add_DragLeave
pub const UI_ELEMENT_REMOVE_DRAG_LEAVE: usize = 52; // IUIElement.remove_DragLeave
pub const UI_ELEMENT_ADD_DRAG_OVER: usize = 53; // IUIElement.add_DragOver
pub const UI_ELEMENT_REMOVE_DRAG_OVER: usize = 54; // IUIElement.remove_DragOver
pub const UI_ELEMENT_ADD_DROP: usize = 55; // IUIElement.add_Drop
pub const UI_ELEMENT_REMOVE_DROP: usize = 56; // IUIElement.remove_Drop
pub const UI_ELEMENT_ADD_HANDLER: usize = 96; // IUIElement.AddHandler
pub const UI_ELEMENT_REMOVE_HANDLER: usize = 97; // IUIElement.RemoveHandler
pub const UI_ELEMENT_STATICS: Guid = Guid::from_u128(0x58d3573b_f52c_45be_988b_a5869564873c); // windows.ui.xaml.h IUIElementStatics
pub const POINTER_PRESSED_EVENT: usize = 9; // IUIElementStatics.get_PointerPressedEvent
pub const POINTER_MOVED_EVENT: usize = 10; // IUIElementStatics.get_PointerMovedEvent
pub const POINTER_RELEASED_EVENT: usize = 11; // IUIElementStatics.get_PointerReleasedEvent
pub const POINTER_CAPTURE_LOST_EVENT: usize = 13; // IUIElementStatics.get_PointerCaptureLostEvent
pub const FRAMEWORK_ELEMENT: Guid = Guid::from_u128(0xa391d09b_4a99_4b7c_9d8d_6fa5d01f6fbf); // windows.ui.xaml.h IFrameworkElement
pub const FRAMEWORK_GET_TAG: usize = 9; // IFrameworkElement.get_Tag
pub const FRAMEWORK_ACTUAL_WIDTH: usize = 13; // IFrameworkElement.get_ActualWidth
pub const FRAMEWORK_ACTUAL_HEIGHT: usize = 14; // IFrameworkElement.get_ActualHeight
pub const FRAMEWORK_FIND_NAME: usize = 51; // IFrameworkElement.FindName
pub const DEPENDENCY_OBJECT: Guid = Guid::from_u128(0x5c526665_f60e_4912_af59_5fe0680f089d); // windows.ui.xaml.h IDependencyObject
pub const VISUAL_TREE_STATICS: Guid = Guid::from_u128(0xe75758c4_d25d_4b1d_971f_596f17f12baa); // windows.ui.xaml.media.h IVisualTreeHelperStatics
pub const VISUAL_TREE_GET_CHILD: usize = 10; // IVisualTreeHelperStatics.GetChild
pub const VISUAL_TREE_CHILDREN_COUNT: usize = 11; // IVisualTreeHelperStatics.GetChildrenCount
pub const GENERAL_TRANSFORM: Guid = Guid::from_u128(0xa06798b7_a2ec_415f_ade2_eade9333f2c7); // windows.ui.xaml.media.h IGeneralTransform
pub const GENERAL_TRANSFORM_BOUNDS: usize = 9; // IGeneralTransform.TransformBounds
pub const PROPERTY_VALUE: Guid = Guid::from_u128(0x4bd682dd_7554_40e9_9a9b_82654ede7e62); // windows.foundation.h IPropertyValue
pub const PROPERTY_VALUE_GET_STRING: usize = 19; // IPropertyValue.GetString
pub const TEXT_BLOCK: Guid = Guid::from_u128(0xae2d9271_3b4a_45fc_8468_f7949548f4d5); // windows.ui.xaml.controls.h ITextBlock
pub const TEXT_BLOCK_FONT_SIZE: usize = 6; // ITextBlock.get_FontSize
pub const TEXT_BLOCK_TEXT: usize = 26; // ITextBlock.get_Text
pub const TEXT_BLOCK_PADDING: usize = 29; // ITextBlock.get_Padding
pub const TEXT_BLOCK_BASELINE: usize = 42; // ITextBlock.get_BaselineOffset
pub const CONTENT_CONTROL: Guid = Guid::from_u128(0xa26dd1dc_cd44_435c_be94_01d6241c231c); // windows.ui.xaml.controls.h IContentControl
pub const CONTENT_CONTROL_PUT_CONTENT: usize = 7; // IContentControl.put_Content
pub const CONTROL: Guid = Guid::from_u128(0xa8912263_2951_4f58_a9c5_5a134eaa7f07); // windows.ui.xaml.controls.h IControl
pub const CONTROL_FONT_SIZE: usize = 6; // IControl.get_FontSize
pub const CONTROL_FOCUS: usize = 46; // IControl.Focus
pub const SCROLL_VIEWER: Guid = Guid::from_u128(0x64e9be00_4dc1_493d_abe7_cbd3c577490d); // windows.ui.xaml.controls.h IScrollViewer
pub const SCROLL_VIEWER_SCROLL_TO_VERTICAL: usize = 59; // IScrollViewer.ScrollToVerticalOffset

// Controls.
pub const BUTTON_BASE: Guid = Guid::from_u128(0xfa002c1a_494e_46cf_91d4_e14a8d798674); // windows.ui.xaml.controls.primitives.h IButtonBase
pub const BUTTON_ADD_CLICK: usize = 14; // IButtonBase.add_Click
pub const BUTTON_REMOVE_CLICK: usize = 15; // IButtonBase.remove_Click
pub const MENU_ITEM: Guid = Guid::from_u128(0x160775cf_b598_44af_a488_c3ce2af6d3f0); // windows.ui.xaml.controls.h IMenuFlyoutItem
pub const MENU_ITEM_ADD_CLICK: usize = 12; // IMenuFlyoutItem.add_Click
pub const MENU_ITEM_REMOVE_CLICK: usize = 13; // IMenuFlyoutItem.remove_Click
pub const TOGGLE_SWITCH: Guid = Guid::from_u128(0x331d8f00_c5f9_46a5_b6c8_ede539304567); // windows.ui.xaml.controls.h IToggleSwitch
pub const TOGGLE_SWITCH_ADD_TOGGLED: usize = 21; // IToggleSwitch.add_Toggled
pub const TOGGLE_SWITCH_REMOVE_TOGGLED: usize = 22; // IToggleSwitch.remove_Toggled
pub const TEXT_BOX: Guid = Guid::from_u128(0xe48f5a8b_1dff_4352_a1f4_e516514ec882); // windows.ui.xaml.controls.h ITextBox
pub const TEXT_BOX_GET_TEXT: usize = 6; // ITextBox.get_Text
pub const TEXT_BOX_PUT_TEXT: usize = 7; // ITextBox.put_Text
pub const TEXT_BOX_GET_SELECTION_LENGTH: usize = 10; // ITextBox.get_SelectionLength
pub const TEXT_BOX_PUT_SELECTION_LENGTH: usize = 11; // ITextBox.put_SelectionLength
pub const TEXT_BOX_GET_SELECTION_START: usize = 12; // ITextBox.get_SelectionStart
pub const TEXT_BOX_PUT_SELECTION_START: usize = 13; // ITextBox.put_SelectionStart

// Events and their arguments.
pub const ROUTED_EVENT_HANDLER: Guid = Guid::from_u128(0xa856e674_b0b6_4bc3_bba8_1ba06e40d4b5); // windows.ui.xaml.h IRoutedEventHandler
pub const POINTER_EVENT_HANDLER: Guid = Guid::from_u128(0xe4385929_c004_4bcf_8970_359486e39f88); // windows.ui.xaml.input.h IPointerEventHandler
pub const KEY_EVENT_HANDLER: Guid = Guid::from_u128(0x7c63d2e5_7a0e_4e12_b96a_7715aa6ff1c8); // windows.ui.xaml.input.h IKeyEventHandler
pub const DRAG_EVENT_HANDLER: Guid = Guid::from_u128(0x2ab1a205_1e73_4bcf_aabc_57b97e21961d); // windows.ui.xaml.h IDragEventHandler
#[cfg(test)]
pub const HANDLER_INVOKE: usize = 3; // IRoutedEventHandler.Invoke
#[cfg(test)]
pub const POINTER_HANDLER_INVOKE: usize = 3; // IPointerEventHandler.Invoke
#[cfg(test)]
pub const KEY_HANDLER_INVOKE: usize = 3; // IKeyEventHandler.Invoke
#[cfg(test)]
pub const DRAG_HANDLER_INVOKE: usize = 3; // IDragEventHandler.Invoke
/// `IReference<PointerEventHandler>`: `AddHandler` takes the delegate boxed.
pub const POINTER_HANDLER_REFERENCE: Guid = Guid::from_u128(0xe33644c8_e0e0_5645_8722_60066c85873c);
pub const POINTER_ARGS: Guid = Guid::from_u128(0xda628f0a_9752_49e2_bde2_49eccab9194d); // windows.ui.xaml.input.h IPointerRoutedEventArgs
pub const POINTER_ARGS_CURRENT_POINT: usize = 10; // IPointerRoutedEventArgs.GetCurrentPoint
pub const POINTER_POINT: Guid = Guid::from_u128(0xe995317d_7296_42d9_8233_c5be73b74a4a); // windows.ui.input.h IPointerPoint
pub const POINTER_POINT_POSITION: usize = 7; // IPointerPoint.get_Position
pub const KEY_ARGS: Guid = Guid::from_u128(0xd4cd3dfe_4079_42e9_a39a_3095d3f049c6); // windows.ui.xaml.input.h IKeyRoutedEventArgs
pub const KEY_ARGS_KEY: usize = 6; // IKeyRoutedEventArgs.get_Key
pub const KEY_ARGS_PUT_HANDLED: usize = 9; // IKeyRoutedEventArgs.put_Handled

// Drag and drop.
pub const DRAG_ARGS2: Guid = Guid::from_u128(0x26336658_2917_411d_bfc3_2f22471cbbe7); // windows.ui.xaml.h IDragEventArgs2
pub const DRAG_ARGS_DATA_VIEW: usize = 6; // IDragEventArgs2.get_DataView
pub const DRAG_ARGS_PUT_ACCEPTED: usize = 10; // IDragEventArgs2.put_AcceptedOperation
pub const DATA_PACKAGE_VIEW: Guid = Guid::from_u128(0x7b840471_5900_4d85_a90b_10cb85fe3552); // windows.applicationmodel.datatransfer.h IDataPackageView
pub const DATA_VIEW_STORAGE_ITEMS: usize = 19; // IDataPackageView.GetStorageItemsAsync
pub const ASYNC_INFO: Guid = Guid::from_u128(0x00000036_0000_0000_c000_000000000046); // asyncinfo.h IAsyncInfo
pub const ASYNC_INFO_STATUS: usize = 7; // IAsyncInfo.get_Status
pub const ASYNC_INFO_ERROR_CODE: usize = 8; // IAsyncInfo.get_ErrorCode
pub const ASYNC_ITEMS_GET_RESULTS: usize = 8; // __FIAsyncOperation_1___FIVectorView_1_Windows__CStorage__CIStorageItem.GetResults
pub const VECTOR_ITEMS_GET_AT: usize = 6; // __FIVectorView_1_Windows__CStorage__CIStorageItem.GetAt
pub const VECTOR_ITEMS_SIZE: usize = 7; // __FIVectorView_1_Windows__CStorage__CIStorageItem.get_Size
pub const STORAGE_ITEM: Guid = Guid::from_u128(0x4207a996_ca2f_42f7_bde8_8b10457a7f30); // windows.storage.h IStorageItem
pub const STORAGE_ITEM_NAME: usize = 11; // IStorageItem.get_Name
pub const STORAGE_ITEM_PATH: usize = 12; // IStorageItem.get_Path

// Pictures of the pages.
pub const RENDER_TARGET_BITMAP: Guid = Guid::from_u128(0x500dee81_893c_4c0a_8fec_4678ac717589); // windows.ui.xaml.media.imaging.h IRenderTargetBitmap
pub const RENDER_PIXEL_WIDTH: usize = 6; // IRenderTargetBitmap.get_PixelWidth
pub const RENDER_PIXEL_HEIGHT: usize = 7; // IRenderTargetBitmap.get_PixelHeight
pub const RENDER_ASYNC: usize = 8; // IRenderTargetBitmap.RenderAsync
pub const RENDER_GET_PIXELS: usize = 10; // IRenderTargetBitmap.GetPixelsAsync
pub const ASYNC_BUFFER_GET_RESULTS: usize = 8; // __FIAsyncOperation_1_Windows__CStorage__CStreams__CIBuffer.GetResults
pub const DATA_READER_STATICS: Guid = Guid::from_u128(0x11fcbfc8_f93a_471b_b121_f379e349313c); // windows.storage.streams.h IDataReaderStatics
pub const DATA_READER_FROM_BUFFER: usize = 6; // IDataReaderStatics.FromBuffer
pub const DATA_READER: Guid = Guid::from_u128(0xe2b50029_b4c1_4314_a4b8_fb813a2f275e); // windows.storage.streams.h IDataReader
pub const DATA_READER_UNCONSUMED: usize = 6; // IDataReader.get_UnconsumedBufferLength
pub const DATA_READER_READ_BYTES: usize = 14; // IDataReader.ReadBytes

// The app theme.
pub const UI_SETTINGS3: Guid = Guid::from_u128(0x03021be4_5254_4781_8194_5168f7d06d7b); // windows.ui.viewmanagement.h IUISettings3
pub const UI_SETTINGS_COLOR: usize = 6; // IUISettings3.GetColorValue

// The multi-select file picker.
pub const FILE_OPEN_DIALOG_CLASS: Guid = Guid::from_u128(0xdc1c5a9c_e88a_4dde_a5a1_60f82a20aef7); // shobjidl_core.h class FileOpenDialog
pub const FILE_OPEN_DIALOG: Guid = Guid::from_u128(0xd57c7288_d4ad_4768_be02_9d969532d960); // shobjidl_core.h IFileOpenDialog
pub const MODAL_SHOW: usize = 3; // IModalWindow.Show
pub const FILE_DIALOG_SET_FILE_TYPES: usize = 4; // IFileDialog.SetFileTypes
pub const FILE_DIALOG_SET_OPTIONS: usize = 9; // IFileDialog.SetOptions
pub const FILE_DIALOG_GET_OPTIONS: usize = 10; // IFileDialog.GetOptions
pub const FILE_DIALOG_SET_TITLE: usize = 17; // IFileDialog.SetTitle
pub const FILE_OPEN_DIALOG_RESULTS: usize = 27; // IFileOpenDialog.GetResults
pub const SHELL_ITEM_ARRAY: Guid = Guid::from_u128(0xb63ea76d_1f85_456f_a19c_48159efa858b); // shobjidl_core.h IShellItemArray
pub const SHELL_ITEM_ARRAY_COUNT: usize = 7; // IShellItemArray.GetCount
pub const SHELL_ITEM_ARRAY_AT: usize = 8; // IShellItemArray.GetItemAt
pub const SHELL_ITEM: Guid = Guid::from_u128(0x43826d1e_e718_42ee_bc55_a1e261c37bfe); // shobjidl_core.h IShellItem
pub const SHELL_ITEM_DISPLAY_NAME: usize = 5; // IShellItem.GetDisplayName

// Diagnostics of the page check.
pub const RESTRICTED_ERROR_INFO: Guid = Guid::from_u128(0x82ba7092_4c88_427d_a7bc_16dd93feb67e); // RestrictedErrorInfo.h IRestrictedErrorInfo
pub const RESTRICTED_ERROR_DETAILS: usize = 3; // IRestrictedErrorInfo.GetErrorDetails
