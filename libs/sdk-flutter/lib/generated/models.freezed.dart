// dart format width=80
// coverage:ignore-file
// GENERATED CODE - DO NOT MODIFY BY HAND
// ignore_for_file: type=lint
// ignore_for_file: unused_element, deprecated_member_use, deprecated_member_use_from_same_package, use_function_type_syntax_for_parameters, unnecessary_const, avoid_init_to_null, invalid_override_different_default_values_named, prefer_expression_function_bodies, annotate_overrides, invalid_annotation_target, unnecessary_question_mark

part of 'models.dart';

// **************************************************************************
// FreezedGenerator
// **************************************************************************

// dart format off
T _$identity<T>(T value) => value;
/// @nodoc
mixin _$LnUrlInfo {

 Object get info;



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is LnUrlInfo&&const DeepCollectionEquality().equals(other.info, info));
}


@override
int get hashCode => Object.hash(runtimeType,const DeepCollectionEquality().hash(info));

@override
String toString() {
  return 'LnUrlInfo(info: $info)';
}


}

/// @nodoc
class $LnUrlInfoCopyWith<$Res>  {
$LnUrlInfoCopyWith(LnUrlInfo _, $Res Function(LnUrlInfo) __);
}


/// @nodoc


class LnUrlInfo_Pay extends LnUrlInfo {
  const LnUrlInfo_Pay({required this.info}): super._();
  

@override final  LnUrlPayInfo info;

/// Create a copy of LnUrlInfo
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$LnUrlInfo_PayCopyWith<LnUrlInfo_Pay> get copyWith => _$LnUrlInfo_PayCopyWithImpl<LnUrlInfo_Pay>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is LnUrlInfo_Pay&&(identical(other.info, info) || other.info == info));
}


@override
int get hashCode => Object.hash(runtimeType,info);

@override
String toString() {
  return 'LnUrlInfo.pay(info: $info)';
}


}

/// @nodoc
abstract mixin class $LnUrlInfo_PayCopyWith<$Res> implements $LnUrlInfoCopyWith<$Res> {
  factory $LnUrlInfo_PayCopyWith(LnUrlInfo_Pay value, $Res Function(LnUrlInfo_Pay) _then) = _$LnUrlInfo_PayCopyWithImpl;
@useResult
$Res call({
 LnUrlPayInfo info
});




}
/// @nodoc
class _$LnUrlInfo_PayCopyWithImpl<$Res>
    implements $LnUrlInfo_PayCopyWith<$Res> {
  _$LnUrlInfo_PayCopyWithImpl(this._self, this._then);

  final LnUrlInfo_Pay _self;
  final $Res Function(LnUrlInfo_Pay) _then;

/// Create a copy of LnUrlInfo
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? info = null,}) {
  return _then(LnUrlInfo_Pay(
info: null == info ? _self.info : info // ignore: cast_nullable_to_non_nullable
as LnUrlPayInfo,
  ));
}


}

/// @nodoc


class LnUrlInfo_Withdraw extends LnUrlInfo {
  const LnUrlInfo_Withdraw({required this.info}): super._();
  

@override final  LnUrlWithdrawInfo info;

/// Create a copy of LnUrlInfo
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$LnUrlInfo_WithdrawCopyWith<LnUrlInfo_Withdraw> get copyWith => _$LnUrlInfo_WithdrawCopyWithImpl<LnUrlInfo_Withdraw>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is LnUrlInfo_Withdraw&&(identical(other.info, info) || other.info == info));
}


@override
int get hashCode => Object.hash(runtimeType,info);

@override
String toString() {
  return 'LnUrlInfo.withdraw(info: $info)';
}


}

/// @nodoc
abstract mixin class $LnUrlInfo_WithdrawCopyWith<$Res> implements $LnUrlInfoCopyWith<$Res> {
  factory $LnUrlInfo_WithdrawCopyWith(LnUrlInfo_Withdraw value, $Res Function(LnUrlInfo_Withdraw) _then) = _$LnUrlInfo_WithdrawCopyWithImpl;
@useResult
$Res call({
 LnUrlWithdrawInfo info
});




}
/// @nodoc
class _$LnUrlInfo_WithdrawCopyWithImpl<$Res>
    implements $LnUrlInfo_WithdrawCopyWith<$Res> {
  _$LnUrlInfo_WithdrawCopyWithImpl(this._self, this._then);

  final LnUrlInfo_Withdraw _self;
  final $Res Function(LnUrlInfo_Withdraw) _then;

/// Create a copy of LnUrlInfo
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? info = null,}) {
  return _then(LnUrlInfo_Withdraw(
info: null == info ? _self.info : info // ignore: cast_nullable_to_non_nullable
as LnUrlWithdrawInfo,
  ));
}


}

/// @nodoc
mixin _$LnUrlPayTarget {





@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is LnUrlPayTarget);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'LnUrlPayTarget()';
}


}

/// @nodoc
class $LnUrlPayTargetCopyWith<$Res>  {
$LnUrlPayTargetCopyWith(LnUrlPayTarget _, $Res Function(LnUrlPayTarget) __);
}


/// @nodoc


class LnUrlPayTarget_LnAddress extends LnUrlPayTarget {
  const LnUrlPayTarget_LnAddress({required this.address}): super._();
  

 final  String address;

/// Create a copy of LnUrlPayTarget
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$LnUrlPayTarget_LnAddressCopyWith<LnUrlPayTarget_LnAddress> get copyWith => _$LnUrlPayTarget_LnAddressCopyWithImpl<LnUrlPayTarget_LnAddress>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is LnUrlPayTarget_LnAddress&&(identical(other.address, address) || other.address == address));
}


@override
int get hashCode => Object.hash(runtimeType,address);

@override
String toString() {
  return 'LnUrlPayTarget.lnAddress(address: $address)';
}


}

/// @nodoc
abstract mixin class $LnUrlPayTarget_LnAddressCopyWith<$Res> implements $LnUrlPayTargetCopyWith<$Res> {
  factory $LnUrlPayTarget_LnAddressCopyWith(LnUrlPayTarget_LnAddress value, $Res Function(LnUrlPayTarget_LnAddress) _then) = _$LnUrlPayTarget_LnAddressCopyWithImpl;
@useResult
$Res call({
 String address
});




}
/// @nodoc
class _$LnUrlPayTarget_LnAddressCopyWithImpl<$Res>
    implements $LnUrlPayTarget_LnAddressCopyWith<$Res> {
  _$LnUrlPayTarget_LnAddressCopyWithImpl(this._self, this._then);

  final LnUrlPayTarget_LnAddress _self;
  final $Res Function(LnUrlPayTarget_LnAddress) _then;

/// Create a copy of LnUrlPayTarget
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? address = null,}) {
  return _then(LnUrlPayTarget_LnAddress(
address: null == address ? _self.address : address // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class LnUrlPayTarget_Domain extends LnUrlPayTarget {
  const LnUrlPayTarget_Domain({required this.domain}): super._();
  

 final  String domain;

/// Create a copy of LnUrlPayTarget
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$LnUrlPayTarget_DomainCopyWith<LnUrlPayTarget_Domain> get copyWith => _$LnUrlPayTarget_DomainCopyWithImpl<LnUrlPayTarget_Domain>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is LnUrlPayTarget_Domain&&(identical(other.domain, domain) || other.domain == domain));
}


@override
int get hashCode => Object.hash(runtimeType,domain);

@override
String toString() {
  return 'LnUrlPayTarget.domain(domain: $domain)';
}


}

/// @nodoc
abstract mixin class $LnUrlPayTarget_DomainCopyWith<$Res> implements $LnUrlPayTargetCopyWith<$Res> {
  factory $LnUrlPayTarget_DomainCopyWith(LnUrlPayTarget_Domain value, $Res Function(LnUrlPayTarget_Domain) _then) = _$LnUrlPayTarget_DomainCopyWithImpl;
@useResult
$Res call({
 String domain
});




}
/// @nodoc
class _$LnUrlPayTarget_DomainCopyWithImpl<$Res>
    implements $LnUrlPayTarget_DomainCopyWith<$Res> {
  _$LnUrlPayTarget_DomainCopyWithImpl(this._self, this._then);

  final LnUrlPayTarget_Domain _self;
  final $Res Function(LnUrlPayTarget_Domain) _then;

/// Create a copy of LnUrlPayTarget
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? domain = null,}) {
  return _then(LnUrlPayTarget_Domain(
domain: null == domain ? _self.domain : domain // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc
mixin _$PaymentDetails {

 Object get data;



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is PaymentDetails&&const DeepCollectionEquality().equals(other.data, data));
}


@override
int get hashCode => Object.hash(runtimeType,const DeepCollectionEquality().hash(data));

@override
String toString() {
  return 'PaymentDetails(data: $data)';
}


}

/// @nodoc
class $PaymentDetailsCopyWith<$Res>  {
$PaymentDetailsCopyWith(PaymentDetails _, $Res Function(PaymentDetails) __);
}


/// @nodoc


class PaymentDetails_Ln extends PaymentDetails {
  const PaymentDetails_Ln({required this.data}): super._();
  

@override final  LnPaymentDetails data;

/// Create a copy of PaymentDetails
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$PaymentDetails_LnCopyWith<PaymentDetails_Ln> get copyWith => _$PaymentDetails_LnCopyWithImpl<PaymentDetails_Ln>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is PaymentDetails_Ln&&(identical(other.data, data) || other.data == data));
}


@override
int get hashCode => Object.hash(runtimeType,data);

@override
String toString() {
  return 'PaymentDetails.ln(data: $data)';
}


}

/// @nodoc
abstract mixin class $PaymentDetails_LnCopyWith<$Res> implements $PaymentDetailsCopyWith<$Res> {
  factory $PaymentDetails_LnCopyWith(PaymentDetails_Ln value, $Res Function(PaymentDetails_Ln) _then) = _$PaymentDetails_LnCopyWithImpl;
@useResult
$Res call({
 LnPaymentDetails data
});




}
/// @nodoc
class _$PaymentDetails_LnCopyWithImpl<$Res>
    implements $PaymentDetails_LnCopyWith<$Res> {
  _$PaymentDetails_LnCopyWithImpl(this._self, this._then);

  final PaymentDetails_Ln _self;
  final $Res Function(PaymentDetails_Ln) _then;

/// Create a copy of PaymentDetails
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? data = null,}) {
  return _then(PaymentDetails_Ln(
data: null == data ? _self.data : data // ignore: cast_nullable_to_non_nullable
as LnPaymentDetails,
  ));
}


}

/// @nodoc


class PaymentDetails_ClosedChannel extends PaymentDetails {
  const PaymentDetails_ClosedChannel({required this.data}): super._();
  

@override final  ClosedChannelPaymentDetails data;

/// Create a copy of PaymentDetails
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$PaymentDetails_ClosedChannelCopyWith<PaymentDetails_ClosedChannel> get copyWith => _$PaymentDetails_ClosedChannelCopyWithImpl<PaymentDetails_ClosedChannel>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is PaymentDetails_ClosedChannel&&(identical(other.data, data) || other.data == data));
}


@override
int get hashCode => Object.hash(runtimeType,data);

@override
String toString() {
  return 'PaymentDetails.closedChannel(data: $data)';
}


}

/// @nodoc
abstract mixin class $PaymentDetails_ClosedChannelCopyWith<$Res> implements $PaymentDetailsCopyWith<$Res> {
  factory $PaymentDetails_ClosedChannelCopyWith(PaymentDetails_ClosedChannel value, $Res Function(PaymentDetails_ClosedChannel) _then) = _$PaymentDetails_ClosedChannelCopyWithImpl;
@useResult
$Res call({
 ClosedChannelPaymentDetails data
});




}
/// @nodoc
class _$PaymentDetails_ClosedChannelCopyWithImpl<$Res>
    implements $PaymentDetails_ClosedChannelCopyWith<$Res> {
  _$PaymentDetails_ClosedChannelCopyWithImpl(this._self, this._then);

  final PaymentDetails_ClosedChannel _self;
  final $Res Function(PaymentDetails_ClosedChannel) _then;

/// Create a copy of PaymentDetails
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? data = null,}) {
  return _then(PaymentDetails_ClosedChannel(
data: null == data ? _self.data : data // ignore: cast_nullable_to_non_nullable
as ClosedChannelPaymentDetails,
  ));
}


}

/// @nodoc
mixin _$ReportIssueRequest {

 ReportPaymentFailureDetails get data;
/// Create a copy of ReportIssueRequest
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ReportIssueRequestCopyWith<ReportIssueRequest> get copyWith => _$ReportIssueRequestCopyWithImpl<ReportIssueRequest>(this as ReportIssueRequest, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is ReportIssueRequest&&(identical(other.data, data) || other.data == data));
}


@override
int get hashCode => Object.hash(runtimeType,data);

@override
String toString() {
  return 'ReportIssueRequest(data: $data)';
}


}

/// @nodoc
abstract mixin class $ReportIssueRequestCopyWith<$Res>  {
  factory $ReportIssueRequestCopyWith(ReportIssueRequest value, $Res Function(ReportIssueRequest) _then) = _$ReportIssueRequestCopyWithImpl;
@useResult
$Res call({
 ReportPaymentFailureDetails data
});




}
/// @nodoc
class _$ReportIssueRequestCopyWithImpl<$Res>
    implements $ReportIssueRequestCopyWith<$Res> {
  _$ReportIssueRequestCopyWithImpl(this._self, this._then);

  final ReportIssueRequest _self;
  final $Res Function(ReportIssueRequest) _then;

/// Create a copy of ReportIssueRequest
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') @override $Res call({Object? data = null,}) {
  return _then(_self.copyWith(
data: null == data ? _self.data : data // ignore: cast_nullable_to_non_nullable
as ReportPaymentFailureDetails,
  ));
}

}


/// @nodoc


class ReportIssueRequest_PaymentFailure extends ReportIssueRequest {
  const ReportIssueRequest_PaymentFailure({required this.data}): super._();
  

@override final  ReportPaymentFailureDetails data;

/// Create a copy of ReportIssueRequest
/// with the given fields replaced by the non-null parameter values.
@override @JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ReportIssueRequest_PaymentFailureCopyWith<ReportIssueRequest_PaymentFailure> get copyWith => _$ReportIssueRequest_PaymentFailureCopyWithImpl<ReportIssueRequest_PaymentFailure>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is ReportIssueRequest_PaymentFailure&&(identical(other.data, data) || other.data == data));
}


@override
int get hashCode => Object.hash(runtimeType,data);

@override
String toString() {
  return 'ReportIssueRequest.paymentFailure(data: $data)';
}


}

/// @nodoc
abstract mixin class $ReportIssueRequest_PaymentFailureCopyWith<$Res> implements $ReportIssueRequestCopyWith<$Res> {
  factory $ReportIssueRequest_PaymentFailureCopyWith(ReportIssueRequest_PaymentFailure value, $Res Function(ReportIssueRequest_PaymentFailure) _then) = _$ReportIssueRequest_PaymentFailureCopyWithImpl;
@override @useResult
$Res call({
 ReportPaymentFailureDetails data
});




}
/// @nodoc
class _$ReportIssueRequest_PaymentFailureCopyWithImpl<$Res>
    implements $ReportIssueRequest_PaymentFailureCopyWith<$Res> {
  _$ReportIssueRequest_PaymentFailureCopyWithImpl(this._self, this._then);

  final ReportIssueRequest_PaymentFailure _self;
  final $Res Function(ReportIssueRequest_PaymentFailure) _then;

/// Create a copy of ReportIssueRequest
/// with the given fields replaced by the non-null parameter values.
@override @pragma('vm:prefer-inline') $Res call({Object? data = null,}) {
  return _then(ReportIssueRequest_PaymentFailure(
data: null == data ? _self.data : data // ignore: cast_nullable_to_non_nullable
as ReportPaymentFailureDetails,
  ));
}


}

// dart format on
